#!/usr/bin/env bash
# Builds the macOS and Linux release files on this Mac and publishes them to the
# GitHub release for the version in Cargo.toml (tag v<version>).
#
# Windows cannot be cross-compiled from macOS (gpui compiles its DirectX shaders
# with fxc.exe, which only runs on a Windows host). Build the .exe on a Windows
# PC and attach it with: gh release upload v<version> whatsapp-broadcaster-windows-x64.exe
#
# Usage: ./scripts/release.sh [--no-publish] [--skip-checks] [--only macos|linux]
#   --no-publish   build into dist/ only; no tag, no upload
#   --skip-checks  skip clippy and tests
#   --only X       build just one platform
#
# Needs: rustup, Docker (for Linux), gh logged in (for publishing).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

publish=1
checks=1
only=""
while [ $# -gt 0 ]; do
  case "$1" in
    --no-publish)  publish=0 ;;
    --skip-checks) checks=0 ;;
    --only)        only="${2:?--only needs macos or linux}"; shift ;;
    -h|--help)     sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)             echo "Unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done
case "$only" in ""|macos|linux) ;; *) echo "--only must be macos or linux" >&2; exit 1 ;; esac
want() { [ -z "$only" ] || [ "$only" = "$1" ]; }

app_name="WhatsApp Broadcaster"
bin="whatsapp-broadcaster"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
tag="v$version"
rust_version="$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)"
dist="$root/dist"

step() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }

if [ "$publish" = 1 ]; then
  gh auth status >/dev/null 2>&1 || { echo "gh is not logged in (run: gh auth login)" >&2; exit 1; }
  [ -z "$(git status --porcelain)" ] || { echo "Working tree is not clean; commit first." >&2; exit 1; }
  if git rev-parse -q --verify "refs/tags/$tag" >/dev/null &&
     [ "$(git rev-parse "$tag^{commit}")" != "$(git rev-parse HEAD)" ]; then
    echo "Tag $tag already exists on another commit. Bump the version in Cargo.toml." >&2
    exit 1
  fi
fi

if [ "$checks" = 1 ]; then
  step "Clippy and tests"
  cargo clippy --workspace --all-targets --locked -- -D warnings
  cargo test --workspace --locked
fi

rm -rf "$dist"
mkdir -p "$dist"

if want macos; then
  step "macOS universal app"
  export MACOSX_DEPLOYMENT_TARGET=11.0
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    rustup target add "$t" >/dev/null
    cargo build --release --locked -p "$bin" --target "$t"
  done

  app="$dist/$app_name.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  lipo -create -output "$app/Contents/MacOS/$bin" \
    "target/aarch64-apple-darwin/release/$bin" \
    "target/x86_64-apple-darwin/release/$bin"
  cp assets/icon.icns "$app/Contents/Resources/icon.icns"
  cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$app_name</string>
  <key>CFBundleDisplayName</key><string>$app_name</string>
  <key>CFBundleIdentifier</key><string>com.rikad.whatsapp-broadcaster</string>
  <key>CFBundleExecutable</key><string>$bin</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>$MACOSX_DEPLOYMENT_TARGET</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
  # Ad-hoc signature: unsigned arm64 apps are reported as "damaged" after download.
  codesign --force --deep --sign - "$app"
  ditto -c -k --keepParent "$app" "$dist/$bin-macos-universal.zip"
  rm -rf "$app"
fi

if want linux; then
  step "Linux x64 (Docker)"
  image="$bin-linux-builder"
  docker build -q -f scripts/linux-builder.Dockerfile --build-arg RUST_VERSION="$rust_version" -t "$image" scripts >/dev/null
  # Registry and target live in named volumes so later builds are incremental.
  docker run --rm \
    -v "$root":/src \
    -v wb-cargo-registry:/usr/local/cargo/registry \
    -v wb-linux-target:/src/target \
    "$image" sh -c "cargo build --release --locked -p $bin --target x86_64-unknown-linux-gnu &&
      cp target/x86_64-unknown-linux-gnu/release/$bin /src/dist/$bin-linux-x64"
  tar -C "$dist" -czf "$dist/$bin-linux-x64.tar.gz" "$bin-linux-x64"
  rm "$dist/$bin-linux-x64"
fi

step "Built"
ls -lh "$dist"

[ "$publish" = 1 ] || exit 0

step "Publishing $tag"
if ! git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  git tag -a "$tag" -m "$tag"
fi
git push origin "$tag"
if gh release view "$tag" >/dev/null 2>&1; then
  gh release upload "$tag" "$dist"/* --clobber
else
  gh release create "$tag" "$dist"/* --title "$tag" --generate-notes --verify-tag
fi
echo
echo "Published $(gh release view "$tag" --json url -q .url)"
echo "Windows is not included; build it on Windows and run: gh release upload $tag whatsapp-broadcaster-windows-x64.exe"
