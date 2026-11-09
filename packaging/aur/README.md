# haven-chat-bin (AUR)

`haven-chat-bin` is the Arch User Repository package for Haven Desktop. It repackages the signed `.deb` attached to each GitHub release, so Arch gets the same binary as Debian and Ubuntu. It is not called `haven-desktop-bin` because that name belongs to an unrelated Matrix client, and it conflicts with `haven-desktop` because both install `/usr/bin/haven-desktop`.

## Install

```bash
yay -S haven-chat-bin
```

Without an AUR helper: `git clone https://aur.archlinux.org/haven-chat-bin.git && cd haven-chat-bin && makepkg -si`.

Updates come through pacman or your AUR helper. The in-app updater sees that the package manager owns the install and leaves updates to it.

## Publishing a release

Once the release's assets are on GitHub, run from this checkout:

```bash
AMNI_DRY_RUN=1 haven-release aur . Haven_<version>_amd64.deb <version>
haven-release aur . Haven_<version>_amd64.deb <version>
```

`haven-release aur` (built from `tools/haven-release.gold`) points `PKGBUILD` at the new deb (`pkgver`, `pkgrel=1`, its SHA-256), test-builds it with `makepkg`, regenerates `.SRCINFO`, clones `ssh://aur@aur.archlinux.org/haven-chat-bin.git`, commits the two files and pushes them. With `AMNI_DRY_RUN=1` it commits in the throwaway clone and does not push. Commit the updated `PKGBUILD` and `.SRCINFO` here afterwards.

The push needs an AUR account whose SSH keys include this machine's public key. For a packaging-only change, edit `PKGBUILD`, raise `pkgrel` by hand, run `makepkg --printsrcinfo > .SRCINFO` and push both files to the AUR repo.
