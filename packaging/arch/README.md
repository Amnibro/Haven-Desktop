# Haven Desktop pacman repository (Arch Linux)

The AUR stopped accepting new accounts, so `haven-chat-bin` cannot be published there. Amni-Scient serves its own signed pacman repository instead, next to the APT and Flatpak repositories on `downloads.amni-scient.com`. It holds the same package as `packaging/aur`: the release `.deb` repackaged by `PKGBUILD`.

## Install (x86_64)

```bash
curl -fsSL https://downloads.amni-scient.com/arch/amni-scient.asc | sudo pacman-key --add -
sudo pacman-key --lsign-key 9535881CC052B33C7D99C359C4C6ED5FBF6A6DBB
printf '\n[amni-scient]\nSigLevel = Required DatabaseRequired\nServer = https://downloads.amni-scient.com/arch/$arch\n' | sudo tee -a /etc/pacman.conf
sudo pacman -Syu haven-chat
```

The first two commands trust the repository key. `pacman-key --finger 9535881CC052B33C7D99C359C4C6ED5FBF6A6DBB` should show `9535 881C C052 B33C 7D99  C359 C4C6 ED5F BF6A 6DBB` and the name "Amni-Scient Flatpak Repo". It is the key that signs the APT and Flatpak repositories too. The `[amni-scient]` section can go anywhere below `[options]` in `/etc/pacman.conf`.

`haven-chat` is a name the package provides; pacman installs `haven-chat-bin`. Use `-Syu`, not `-Sy haven-chat`, so the database never gets ahead of the rest of the system. Haven then updates with the rest of the system (`sudo pacman -Syu`). The in-app updater sees that pacman owns the install and leaves updates to it.

The package conflicts with `haven-desktop` (the Amni OS package) and with `haven-chat`, because both install `/usr/bin/haven-desktop`.

ARM64 Arch (Arch Linux ARM) gets the same repository under `aarch64` once the ARM64 `.deb` ships; the `$arch` in the `Server` line already selects it.

## Layout

```
arch/
  amni-scient.asc                     public key (armored)
  x86_64/
    amni-scient.db                    signed package database (real file, not a link)
    amni-scient.db.sig
    amni-scient.db.tar.zst            same bytes, the name repo-add uses
    amni-scient.db.tar.zst.sig
    amni-scient.files, .files.sig, .files.tar.zst, .files.tar.zst.sig
    haven-chat-bin-<ver>-1-x86_64.pkg.tar.zst
    haven-chat-bin-<ver>-1-x86_64.pkg.tar.zst.sig
  aarch64/                            same files once ARM64 packages exist
```

`repo-add` links `amni-scient.db` to `amni-scient.db.tar.zst`; `haven-release pacman` replaces the links with real copies so any static file host or sync tool serves them. The newest three package versions stay so a client in the middle of an update still finds its file.

## Publishing a release

After `tools/release-linux.sh` has produced the `.deb` (it runs this step itself after `publish-apt.py`), or by hand:

```bash
haven-release pacman . ~/.cache/amni-build/haven-desktop-rel/<version>/Haven_<version>_amd64.deb <version>
```

It runs on this Arch machine because it needs `makepkg` and `repo-add`. It points `packaging/aur/PKGBUILD` at the new deb (`pkgver`, `pkgrel=1`, SHA-256), runs `makepkg`, signs the package with `gpg --detach-sign`, replaces the package's entry in the database with `repo-add --sign`, signs the database and files list, checks the database signature, prunes old versions and exports the public key. Output goes to `~/ai/Platform/Amni-OS/amni-bake/releases/arch`, which `amni-downloads.service` serves at `https://downloads.amni-scient.com/arch/`. Commit the bumped `PKGBUILD` afterwards.

Environment: `HAVEN_ARCH_DIR` (output folder), `HAVEN_ARCH_GNUPG` (GnuPG home holding the signing key, default `~/.config/amni/signing/flatpak-gnupg`, the key `publish-apt.py` and `publish-flatpak.sh` use). Never rotate that key: installed systems pin it.

Serving notes: `download-server.js` sends `no-cache` for `.db` and `.files`, and `immutable` for `.pkg.tar.*`. Package file names carry the version and are served `immutable`, so republishing the same version replaces a file that caches may keep. Publish every change under a new version.

## ARM64 (when `fix/b1-arm64-current` is merged and released)

Its `release-linux.sh --arch aarch64` produces `Haven_<ver>_arm64.deb`. Add to `PKGBUILD`: `arch=('x86_64' 'aarch64')`, `source_aarch64`, `sha256sums_aarch64` for `Haven_${pkgver}_arm64.deb`, and make `haven-release pacman` pick the folder (`x86_64` or `aarch64`) and package suffix from the deb's architecture. No client change is needed.
