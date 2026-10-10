# Arch Linux (AUR) package

`haven-chat-bin` repackages the `.deb` that CI attaches to each GitHub release.
It is named `haven-chat-bin` because `haven-desktop-bin` on the AUR is a different client.

## Install

```bash
yay -S haven-chat-bin      # or: paru -S haven-chat-bin
```

Updates arrive through your AUR helper like any other package. The in-app updater
only runs from the AppImage, so a pacman install never prompts you to update itself.

## Build locally

```bash
cd packaging/aur
makepkg -si
```

## Publishing

The `publish-aur` job in `.github/workflows/build-tauri.yml` runs after each release.
It sets `pkgver` to the version in `src-tauri/tauri.conf.json`, recomputes the checksums,
regenerates `.SRCINFO` and pushes to `ssh://aur@aur.archlinux.org/haven-chat-bin.git`.

One-time setup:

1. Create an AUR account and add an SSH public key to it.
2. Add the matching private key as the repository secret `AUR_SSH_PRIVATE_KEY`.
3. Optionally set `AUR_USERNAME` and `AUR_EMAIL` repository variables for the commit author.

Without the secret, the job skips itself. The first push to the AUR creates the package.
