#!/usr/bin/env bash
# Regenerates assets/icon.png, icon.ico and icon.icns from the SVG sources.
# Requires Google Chrome (SVG rendering) and ImageMagick (`brew install imagemagick`).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
assets="$root/assets"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
[ -x "$chrome" ] || { echo "Chrome not found at $chrome (set CHROME=...)"; exit 1; }

render() { # svg -> 1024px png
  "$chrome" --headless --disable-gpu --hide-scrollbars \
    --default-background-color=00000000 --window-size=1024,1024 \
    --screenshot="$2" "file://$1" >/dev/null 2>&1
}

render "$assets/icon.svg"       "$work/full.png"
render "$assets/icon-small.svg" "$work/small.png"
render "$assets/icon-tiny.svg"  "$work/tiny.png"

# Pick the variant tuned for each pixel size: detail scales with room.
src_for() {
  case "$1" in
    16|20|24) echo "$work/tiny.png" ;;
    32|48)    echo "$work/small.png" ;;
    *)        echo "$work/full.png" ;;
  esac
}

for s in 16 24 32 48 64 128 256 512 1024; do
  magick "$(src_for $s)" -filter Lanczos -resize "${s}x${s}" -strip "$work/$s.png"
done

cp "$work/1024.png" "$assets/icon.png"
cp "$work/512.png"  "$assets/icon-512.png"

# Windows executable icon
magick "$work/16.png" "$work/24.png" "$work/32.png" "$work/48.png" \
       "$work/64.png" "$work/128.png" "$work/256.png" "$assets/icon.ico"

# macOS icon bundle
set="$work/icon.iconset"; mkdir -p "$set"
cp "$work/16.png"   "$set/icon_16x16.png"
cp "$work/32.png"   "$set/icon_16x16@2x.png"
cp "$work/32.png"   "$set/icon_32x32.png"
cp "$work/64.png"   "$set/icon_32x32@2x.png"
cp "$work/128.png"  "$set/icon_128x128.png"
cp "$work/256.png"  "$set/icon_128x128@2x.png"
cp "$work/256.png"  "$set/icon_256x256.png"
cp "$work/512.png"  "$set/icon_256x256@2x.png"
cp "$work/512.png"  "$set/icon_512x512.png"
cp "$work/1024.png" "$set/icon_512x512@2x.png"
iconutil -c icns "$set" -o "$assets/icon.icns"

echo "Wrote icon.png, icon-512.png, icon.ico, icon.icns to $assets"
