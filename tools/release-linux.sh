#!/bin/bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
V=$(node -p "require('./src-tauri/tauri.conf.json').version")
OUT=${HAVEN_REL_DIR:-$HOME/.cache/amni-build/haven-desktop-rel}/$V
DEB= UPLOAD=0 ARCH=${HAVEN_LINUX_ARCH:-x86_64}
while (($#)); do case $1 in --deb) DEB=$2; shift 2;; --arch) ARCH=$2; shift 2;; --upload) UPLOAD=1; shift;; *) echo "usage: release-linux.sh [--arch x86_64|aarch64] [--deb file.deb] [--upload]"; exit 2;; esac; done
case $ARCH in x86_64) D=amd64 TARGET= IB=haven-linux-builder MA=x86_64-linux-gnu;; aarch64) D=arm64 TARGET=aarch64-unknown-linux-gnu IB=haven-linux-builder-arm64 MA=aarch64-linux-gnu;; *) echo "unknown arch $ARCH"; exit 2;; esac
IMG=${HAVEN_FLATPAK_IMAGE:-ghcr.io/flathub-infra/flatpak-github-actions:gnome-49}
mkdir -p "$OUT"
if [[ -z $DEB ]]; then
  [[ -n ${TAURI_SIGNING_PRIVATE_KEY:-} ]] || { echo "TAURI_SIGNING_PRIVATE_KEY is not set; the updater needs signed AppImages"; exit 1; }
  podman build -q --build-arg DEB_ARCH=$D -t $IB -f tools/linux-builder.Containerfile tools >/dev/null
  CACHE=${HAVEN_LINUX_CACHE:-$HOME/.cache/amni-build/haven-linux}
  mkdir -p "$CACHE/target" "$CACHE/cargo" "$CACHE/npm" "$CACHE/tauri"
  podman run --rm -v "$ROOT:/src" -v "$CACHE/target:/target" -v "$CACHE/cargo:/root/.cargo/registry" -v "$CACHE/npm:/root/.npm" -v "$CACHE/tauri:/root/.cache/tauri" -e CARGO_TARGET_DIR=/target -e DEB_HOST_MULTIARCH=$MA -e TAURI_SIGNING_PRIVATE_KEY -e TAURI_SIGNING_PRIVATE_KEY_PASSWORD $IB sh tools/linux-bundle.sh $TARGET
  B=$CACHE/target${TARGET:+/$TARGET}/release/bundle
  cp "$B"/appimage/*.AppImage "$OUT/Haven_${V}_$D.AppImage"
  cp "$B"/appimage/*.AppImage.sig "$OUT/Haven_${V}_$D.AppImage.sig"
  DEB=$(ls -t "$B"/deb/*.deb | head -1)
fi
cp "$DEB" "$OUT/Haven_${V}_$D.deb"
[[ -n ${TAURI_SIGNING_PRIVATE_KEY:-} ]] && npx tauri signer sign "$OUT/Haven_${V}_$D.deb"
cp "$DEB" flatpak/haven.deb
[[ -d flatpak/shared-modules ]] || git clone --depth 1 https://github.com/flathub/shared-modules flatpak/shared-modules
META=flatpak/com.haven.desktop.metainfo.xml
grep -q "version=\"$V\"" "$META" || sed -i "s#<releases>#<releases>\n    <release version=\"$V\" date=\"$(date +%F)\"/>#" "$META"
FPU=${HAVEN_FLATPAK_USER:-$HOME/.cache/amni-build/haven-flatpak-user}
mkdir -p "$FPU"
podman run --rm --privileged --platform linux/amd64 -v "$ROOT:/src" -v "$OUT:/out" -v "$FPU:/root/.local/share/flatpak" -w /src "$IMG" sh -c "flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo && flatpak-builder --user --arch=$ARCH --install-deps-from=flathub --force-clean --disable-rofiles-fuse --state-dir=/out/.flatpak-builder --repo=/out/flatpak-build-repo /out/flatpak-build flatpak/com.haven.desktop.yml && flatpak build-bundle --arch=$ARCH /out/flatpak-build-repo /out/Haven_${V}_$ARCH.flatpak com.haven.desktop && ! flatpak build /out/flatpak-build ldd /app/bin/haven-desktop | grep 'not found'"
HAVEN_LINUX_ARCH=$ARCH "$ROOT/tools/publish-flatpak.sh" "$OUT/Haven_${V}_$ARCH.flatpak"
python3 -I "$ROOT/tools/publish-apt.py" "$OUT/Haven_${V}_$D.deb"
FP=${HAVEN_FLATPAK_DIR:-$HOME/ai/Platform/Amni-OS/amni-bake/releases/flatpak}
cp "$FP/Haven-Desktop-$ARCH.flatpak" "$FP/haven.flatpakref" "$OUT/"
cp "$OUT/Haven_${V}_$D.deb" "$OUT/Haven-Desktop-$D.deb"
[[ -f $OUT/Haven_${V}_$D.AppImage ]] && cp "$OUT/Haven_${V}_$D.AppImage" "$OUT/Haven-Desktop-$ARCH.AppImage"
DL=${HAVEN_DL_DIR:-$HOME/ai/Platform/Amni-OS/amni-bake/releases/haven-desktop}
mkdir -p "$DL"
cp "$OUT"/Haven-Desktop-* "$DL/"
rm -rf "$OUT/flatpak-build" "$OUT/.flatpak-builder" "$OUT/flatpak-build-repo" flatpak/haven.deb
if ((UPLOAD)); then
  TAG=v$V-tauri
  gh release view "$TAG" >/dev/null 2>&1 || { echo "release $TAG does not exist yet; create it first (creating a v* tag starts .github/workflows/build-tauri.yml)"; exit 1; }
  mapfile -t FILES < <(find "$OUT" -maxdepth 1 -type f \( -name '*.AppImage' -o -name '*.AppImage.sig' -o -name '*.deb' -o -name '*.deb.sig' -o -name '*.flatpak' -o -name '*.flatpakref' \))
  gh release upload "$TAG" "${FILES[@]}" --clobber
fi
echo "Linux release $V in $OUT"
