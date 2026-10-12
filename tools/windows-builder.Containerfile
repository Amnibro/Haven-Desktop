FROM docker.io/library/debian:trixie-slim
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends nsis clang lld llvm curl ca-certificates nodejs npm build-essential pkg-config git file p7zip-full && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --target x86_64-pc-windows-msvc && cargo install cargo-xwin --locked && rm -rf /opt/cargo/registry /opt/cargo/git
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends libayatana-appindicator3-dev && rm -rf /var/lib/apt/lists/*
