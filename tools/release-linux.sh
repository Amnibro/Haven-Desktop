#!/bin/bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
V=$(node -p "require('./src-tauri/tauri.conf.json').version")
OUT=${HAVEN_REL_DIR:-$HOME/.cache/amni-build/haven-desktop-rel}/$V
DEB= UPLOAD=0
while (($#)); do case $1 in --deb) DEB=$2; shift 2;; --upload) UPLOAD=1; shift;; *) echo "usage: release-linux.sh [--deb file.deb] [--upload]"; exit 2;; esac; done
IMG=${HAVEN_FLATPAK_IMAGE:-ghcr.io/flathub-infra/flatpak-github-actions:gnome-49}
mkdir -p "$OUT"
if [[ -z $DEB ]]; then
  [[ -n ${TAURI_SIGNING_PRIVATE_KEY:-} ]] || { echo "TAURI_SIGNING_PRIVATE_KEY is not set; the updater needs signed AppImages"; exit 1; }
  podman build -q -t haven-linux-builder -f tools/linux-builder.Containerfile tools >/dev/null
  CACHE=${HAVEN_LINUX_CACHE:-$HOME/.cache/amni-build/haven-linux}
  mkdir -p "$CACHE/target" "$CACHE/cargo" "$CACHE/npm"
  podman run --rm -v "$ROOT:/src" -v "$CACHE/target:/target" -v "$CACHE/cargo:/root/.cargo/registry" -v "$CACHE/npm:/root/.npm" -e CARGO_TARGET_DIR=/target -e TAURI_SIGNING_PRIVATE_KEY -e TAURI_SIGNING_PRIVATE_KEY_PASSWORD haven-linux-builder sh -c "npm ci && npx tauri build --bundles appimage,deb"
  B=$CACHE/target/release/bundle
  cp "$B"/appimage/*.AppImage "$OUT/Haven_${V}_amd64.AppImage"
  cp "$B"/appimage/*.AppImage.sig "$OUT/Haven_${V}_amd64.AppImage.sig"
  DEB=$(ls -t "$B"/deb/*.deb | head -1)
fi
cp "$DEB" "$OUT/Haven_${V}_amd64.deb"
cp "$DEB" flatpak/haven.deb
[[ -d flatpak/shared-modules ]] || git clone --depth 1 https://github.com/flathub/shared-modules flatpak/shared-modules
META=flatpak/com.haven.desktop.metainfo.xml
grep -q "version=\"$V\"" "$META" || sed -i "s#<releases>#<releases>\n    <release version=\"$V\" date=\"$(date +%F)\"/>#" "$META"
podman run --rm --privileged -v "$ROOT:/src" -v "$OUT:/out" -w /src "$IMG" sh -c "flatpak-builder --force-clean --disable-rofiles-fuse --state-dir=/out/.flatpak-builder --repo=/out/flatpak-build-repo /out/flatpak-build flatpak/com.haven.desktop.yml && flatpak build-bundle /out/flatpak-build-repo /out/Haven_${V}_x86_64.flatpak com.haven.desktop && ! flatpak build /out/flatpak-build ldd /app/bin/haven-desktop | grep 'not found'"
"$ROOT/tools/publish-flatpak.sh" "$OUT/Haven_${V}_x86_64.flatpak"
python3 -I "$ROOT/tools/publish-apt.py" "$OUT/Haven_${V}_amd64.deb"
FP=${HAVEN_FLATPAK_DIR:-$HOME/ai/amni-bake/releases/flatpak}
cp "$FP/Haven-Desktop-x86_64.flatpak" "$FP/haven.flatpakref" "$OUT/"
cp "$OUT/Haven_${V}_amd64.deb" "$OUT/Haven-Desktop-amd64.deb"
[[ -f $OUT/Haven_${V}_amd64.AppImage ]] && cp "$OUT/Haven_${V}_amd64.AppImage" "$OUT/Haven-Desktop-x86_64.AppImage"
DL=${HAVEN_DL_DIR:-$HOME/ai/amni-bake/releases/haven-desktop}
mkdir -p "$DL"
cp "$OUT"/Haven-Desktop-* "$DL/"
rm -rf "$OUT/flatpak-build" "$OUT/.flatpak-builder" "$OUT/flatpak-build-repo" flatpak/haven.deb
if ((UPLOAD)); then
  TAG=v$V-tauri
  gh release view "$TAG" >/dev/null 2>&1 || { echo "release $TAG does not exist yet; create it first (creating a v* tag starts .github/workflows/build-tauri.yml)"; exit 1; }
  mapfile -t FILES < <(find "$OUT" -maxdepth 1 -type f \( -name '*.AppImage' -o -name '*.AppImage.sig' -o -name '*.deb' -o -name '*.flatpak' -o -name '*.flatpakref' \))
  gh release upload "$TAG" "${FILES[@]}" --clobber
fi
echo "Linux release $V in $OUT"
