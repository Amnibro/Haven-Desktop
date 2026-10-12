#!/bin/bash
set -euo pipefail
BUNDLE=${1:?usage: publish-flatpak.sh <Haven.flatpak> [out-dir]}
OUT=${2:-$HOME/ai/amni-bake/releases/flatpak}
URL=${HAVEN_FLATPAK_URL:-https://downloads.amni-scient.com/flatpak}
export GNUPGHOME=${HAVEN_FLATPAK_GNUPG:-$HOME/.config/amni/signing/flatpak-gnupg}
KEY=$(gpg --list-keys --with-colons | awk -F: '/^fpr/{print $10; exit}')
APP=com.haven.desktop REF=app/com.haven.desktop/x86_64/stable REPO=$OUT/repo
mkdir -p "$OUT"
[[ -d $REPO/objects ]] || ostree init --mode=archive-z2 --repo="$REPO"
flatpak build-import-bundle --ref="$REF.import" "$REPO" "$BUNDLE"
flatpak build-commit-from --src-ref="$REF.import" --gpg-sign="$KEY" --gpg-homedir="$GNUPGHOME" "$REPO" "$REF"
ostree --repo="$REPO" refs --delete "$REF.import"
flatpak build-update-repo --title="Amni-Scient" --default-branch=stable --generate-static-deltas --prune --prune-depth=3 --gpg-sign="$KEY" --gpg-homedir="$GNUPGHOME" "$REPO"
GPGKEY=$(gpg --export "$KEY" | base64 -w0)
printf '[Flatpak Repo]\nTitle=Amni-Scient\nUrl=%s/repo/\nHomepage=https://amni-scient.com/amni-haven.html\nComment=Haven Desktop and other Amni-Scient apps\nIcon=%s/haven.png\nGPGKey=%s\n' "$URL" "$URL" "$GPGKEY" > "$OUT/amni-scient.flatpakrepo"
printf '[Flatpak Ref]\nName=%s\nBranch=stable\nTitle=Haven\nUrl=%s/repo/\nSuggestRemoteName=amni-scient\nHomepage=https://amni-scient.com/amni-haven.html\nIcon=%s/haven.png\nRuntimeRepo=https://dl.flathub.org/repo/flathub.flatpakrepo\nIsRuntime=false\nGPGKey=%s\n' "$APP" "$URL" "$URL" "$GPGKEY" > "$OUT/haven.flatpakref"
flatpak build-bundle --repo-url="$URL/repo/" --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo --gpg-keys=<(gpg --export "$KEY") "$REPO" "$OUT/Haven-Desktop-x86_64.flatpak" "$APP" stable
gpg --export --armor "$KEY" > "$OUT/amni-scient-flatpak.asc"
echo "published $(ostree --repo="$REPO" log "$REF" | head -1) to $OUT"
