# Haven Desktop on Flathub

Files for a from-source Flathub submission of Haven Desktop (Tauri). The older `flatpak/com.haven.desktop.yml` repackages the `.deb` with `--device=all`, which Flathub does not accept. This manifest builds from the tagged source instead.

## Decisions needed before submitting

- **App ID.** The Tauri identifier `com.haven.desktop` implies the domain haven.com, which the project does not own, and Flathub rejects it. This manifest proposes `io.github.Amnibro.Haven-Desktop` (the Flathub linter checks that `github.com/Amnibro/Haven-Desktop` exists; the check passes for this ID and fails for `HavenDesktop` and `Haven_Desktop`). If the project prefers its own domain, use `com.haven_app.Desktop` (haven-app.com) and rename the three `io.github.Amnibro.Haven-Desktop.*` files, the `id`, the `launchable` and the icon names to match. The Flathub submitter must be able to prove ownership of the chosen ID.
- **Source repository.** The manifest builds the `v2.7.2-tauri` tag from `https://github.com/Amnibro/Haven-Desktop.git`. The tag lives on that repository only. Point `url` at `ancsemi/Haven-Desktop` once the tag exists there.
- **Who submits.** Flathub asks for a pull request against `flathub/flathub` (branch `new-pr`) from the maintainer account, and the maintainer then owns the `flathub/<id>` repository.

## Files

- `io.github.Amnibro.Haven-Desktop.yml` manifest (GNOME 51 runtime, `rust-stable` and `node22` SDK extensions, offline build)
- `io.github.Amnibro.Haven-Desktop.desktop`, `io.github.Amnibro.Haven-Desktop.metainfo.xml`
- `cargo-sources.json` generated from `src-tauri/Cargo.lock` with `flatpak-cargo-generator.py`
- `node-sources.json` generated from `package-lock.json` with `flatpak-node-generator npm`
- `flathub.json` limits builds to x86_64 and aarch64
- `shared-modules` is a git submodule of `https://github.com/flathub/shared-modules.git`, added in the Flathub repository (it provides `libayatana-appindicator`)

Regenerate both source lists whenever `Cargo.lock` or `package-lock.json` changes, and bump `tag`, `commit` and the metainfo `<release>` entry for each release.

## Sandbox permissions

- `--share=network` reaches Haven servers.
- `--socket=wayland`, `--socket=fallback-x11`, `--device=dri` draw the window.
- `--socket=pulseaudio` plays call audio and records the microphone through PipeWire's PulseAudio layer.
- `--filesystem=xdg-download` saves downloaded files.
- `--talk-name=org.kde.StatusNotifierWatcher` and `--filesystem=xdg-run/tray-icon:create` show the tray icon (the tray library writes icon files to `$XDG_RUNTIME_DIR/tray-icon`).
- `--talk-name=org.freedesktop.Notifications` sends desktop notifications. The app sends them with notify-rust straight to that name, and inside the sandbox the call fails with ServiceUnknown without this permission (checked with gdbus).
- The camera and the screen-capture picker go through portals and need no extra permission.
- `--device=all` is not requested.

## Before the first Flathub build

- The metainfo screenshot points at `packaging/flathub/screenshots/welcome.png` on the `main` branch of `Amnibro/Haven-Desktop`. The linter reports `appstream-missing-screenshots` until this folder is merged and pushed to GitHub.
- Local build check: from this folder, `flatpak-builder --user --install-deps-from=flathub --force-clean build io.github.Amnibro.Haven-Desktop.yml` after `git submodule add https://github.com/flathub/shared-modules.git shared-modules`. The manifest, the installed app and the sandbox were checked in a GNOME 51 container on 2026-11-19; see the commit message.
