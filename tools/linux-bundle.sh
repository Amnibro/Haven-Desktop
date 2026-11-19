#!/bin/sh
set -e
T=${1:-}
build() { npx tauri build --bundles appimage,deb ${T:+--target $T}; }
unmark() { for f in /root/.cache/tauri/*.AppImage; do [ -f "$f" ] && printf '\0\0\0' | dd of="$f" bs=1 seek=8 count=3 conv=notrunc 2>/dev/null || :; done; }
npm ci
if [ -z "$T" ]; then build; else unmark; build || { unmark; build; }; fi
