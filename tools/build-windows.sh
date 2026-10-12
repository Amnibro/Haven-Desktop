#!/bin/bash
set -e
W=~/.cache/amni-build/win
SRC=${1:-$(cd "$(dirname "$0")/.." && pwd)}
mkdir -p $W/cargo-registry $W/cargo-git $W/xwin $W/target $W/npm $W/tauri-cache
podman build -q --platform linux/amd64 -t localhost/haven-win-build:latest -f "$SRC/tools/windows-builder.Containerfile" "$SRC/tools" >/dev/null
podman run --rm --name haven-win-build -v "$SRC":/src -v $W/cargo-registry:/opt/cargo/registry -v $W/cargo-git:/opt/cargo/git -v $W/xwin:/cache/xwin -v $W/target:/cache/target -v $W/npm:/root/.npm -v $W/tauri-cache:/root/.cache/tauri -e XWIN_CACHE_DIR=/cache/xwin -e CARGO_TARGET_DIR=/cache/target -e XWIN_ACCEPT_LICENSE=1 -w /src localhost/haven-win-build:latest bash -c 'npm ci --no-audit --no-fund && npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis --config "{\"bundle\":{\"createUpdaterArtifacts\":false}}"'
V=$(node -p "require('$SRC/src-tauri/tauri.conf.json').version")
mkdir -p ~/.cache/amni-build/haven-desktop-rel/$V
cp $W/target/x86_64-pc-windows-msvc/release/bundle/nsis/Haven_${V}_x64-setup.exe ~/.cache/amni-build/haven-desktop-rel/$V/
