FROM docker.io/library/ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive PATH=/root/.cargo/bin:$PATH APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=true CXX=g++
RUN apt-get update && apt-get install -y build-essential pkg-config curl wget file patchelf ca-certificates git libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libpulse-dev libxdo-dev xdg-utils && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && apt-get install -y nodejs && curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal && rm -rf /var/lib/apt/lists/*
WORKDIR /src
