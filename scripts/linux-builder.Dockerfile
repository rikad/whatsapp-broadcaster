# Builds the Linux x64 binary. Used by scripts/release.sh.
# On an arm64 host (Apple Silicon) this cross-compiles to x86_64 instead of
# running the whole build under emulation, which would be many times slower.
FROM debian:bookworm

ARG RUST_VERSION

RUN set -eux; \
    host="$(dpkg --print-architecture)"; \
    if [ "$host" != amd64 ]; then dpkg --add-architecture amd64; fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates curl pkg-config clang; \
    if [ "$host" != amd64 ]; then \
      apt-get install -y --no-install-recommends gcc-x86-64-linux-gnu g++-x86-64-linux-gnu libc6-dev-amd64-cross; \
    else \
      apt-get install -y --no-install-recommends gcc g++ libc6-dev; \
    fi; \
    apt-get install -y --no-install-recommends \
      libxkbcommon-dev:amd64 libxkbcommon-x11-dev:amd64 libwayland-dev:amd64 \
      libxcb1-dev:amd64 libx11-xcb-dev:amd64 libfontconfig-dev:amd64 \
      libfreetype-dev:amd64 libvulkan-dev:amd64; \
    rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
      --default-toolchain "$RUST_VERSION" --target x86_64-unknown-linux-gnu

# Only take effect when cross-compiling; harmless on an amd64 host.
ENV CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
    CXX_x86_64_unknown_linux_gnu=x86_64-linux-gnu-g++ \
    AR_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ar \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    PKG_CONFIG_ALLOW_CROSS=1 \
    PKG_CONFIG_LIBDIR=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig

WORKDIR /src
