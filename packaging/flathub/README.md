# Haven Desktop on Flathub

Files for a from-source Flathub submission of Haven Desktop (Tauri). The older `flatpak/com.haven.desktop.yml` repackages the `.deb` with `--device=all`, which Flathub does not accept. This manifest builds the tagged source with no network access at build time.

## App ID

`com.amni_scient.Haven`. Flathub accepts an ID only when the submitter controls the matching domain or code host. `com.haven.desktop`, the Tauri identifier, implies haven.com, which the project does not own. A hyphen in a domain becomes an underscore in the ID, so this one is `amni-scient.com`.

- Verify the ID after the app is accepted by serving the token Flathub shows as `https://amni-scient.com/.well-known/org.flathub.VerifiedApps.txt`.
- The fallback with no domain proof is `io.github.Amnibro.Haven-Desktop` (the linter confirms `github.com/Amnibro/Haven-Desktop` exists). To use it, rename the three `com.amni_scient.Haven.*` files and replace the ID in the manifest, desktop file and metainfo.
- Inside the sandbox the single-instance lock must live under the app's own D-Bus name. `src-tauri/src/lib.rs` now reads `FLATPAK_ID` for it (outside Flatpak it stays `com.haven.desktop`, so the deb, AppImage and Arch package behave as before).

## Files

- `com.amni_scient.Haven.yml` manifest (GNOME 51 runtime, `rust-stable` and `node22` SDK extensions, offline build)
- `com.amni_scient.Haven.desktop`, `com.amni_scient.Haven.metainfo.xml`
- `single-instance-flatpak-id.patch` the `lib.rs` change above, applied on top of the `v2.7.2-tauri` tag
- `cargo-sources.json` generated from `src-tauri/Cargo.lock` with `flatpak-cargo-generator.py`
- `node-sources.json` generated from `package-lock.json` with `flatpak-node-generator npm`
- `flathub.json` limits builds to x86_64 and aarch64
- `screenshots/welcome.png` the screenshot the metainfo points at
- `shared-modules` is a git submodule of `https://github.com/flathub/shared-modules.git`, added in the Flathub repository (it provides `libayatana-appindicator`)

## Each release

1. Regenerate both source lists when `Cargo.lock` or `package-lock.json` changed.
2. Bump `tag` and `commit` in the manifest and add the metainfo `<release>` entry.
3. Once a tag contains the `lib.rs` change (anything after 2.7.2), delete `single-instance-flatpak-id.patch` and its manifest entry.

## Sandbox permissions

- `--share=network` reaches Haven servers.
- `--share=ipc`, `--socket=wayland`, `--socket=fallback-x11`, `--device=dri` draw the window.
- `--socket=pulseaudio` plays call audio and records the microphone through PipeWire's PulseAudio layer.
- `--filesystem=xdg-download` saves downloaded files.
- `--talk-name=org.kde.StatusNotifierWatcher` and `--filesystem=xdg-run/tray-icon:create` show the tray icon (the tray library writes icon files to `$XDG_RUNTIME_DIR/tray-icon`).
- `--talk-name=org.freedesktop.Notifications` sends desktop notifications. The app sends them with notify-rust straight to that name, and inside the sandbox the call fails with ServiceUnknown without this permission.
- The camera and the screen-capture picker go through portals and need no extra permission.
- `--device=all` is not requested.

## Moving existing Flatpak users

Installs of `com.haven.desktop` came from the Amni-Scient Flatpak remote (`downloads.amni-scient.com`), not Flathub. After `com.amni_scient.Haven` is on Flathub, publish it in that remote too and mark the old ref as superseded:

```bash
flatpak build-commit-from --src-repo=<repo with the new id> --src-ref=app/com.amni_scient.Haven/x86_64/stable <own repo> app/com.amni_scient.Haven/x86_64/stable
flatpak build-commit-from --src-ref=app/com.haven.desktop/x86_64/stable --end-of-life='Haven Desktop moved to com.amni_scient.Haven' --end-of-life-rebase=com.haven.desktop=com.amni_scient.Haven <own repo> app/com.haven.desktop/x86_64/stable
flatpak build-update-repo <own repo>
```

`flatpak update` then installs the new app, uninstalls the old one and carries `~/.var/app/com.haven.desktop` over to the new ID (checked locally: the old app's data folder became a link to the new one and the app saw the old files). The new ID must be in the same remote as the old one, because the rebase only searches the remote the old app came from. Flathub never had the old ID, so nothing changes there.

## Checks

- `flatpak-builder-lint manifest com.amni_scient.Haven.yml` is clean.
- `flatpak-builder-lint appstream` and the repo and builddir checks need the screenshot URL in the metainfo to answer. It points at this folder on the `main` branch of `Amnibro/Haven-Desktop`, so they report `appstream-missing-screenshots` / `screenshot-image-not-found` until `screenshots/welcome.png` is pushed there. Flathub's own build mirrors the screenshot into its media store, and its linter then passes.
- Local build: `git submodule add https://github.com/flathub/shared-modules.git shared-modules`, then `flatpak-builder --user --install-deps-from=flathub --force-clean build com.amni_scient.Haven.yml`.
- Checked on 2026-11-24 in the `ghcr.io/flathub-infra/flatpak-github-actions:gnome-49` container (flatpak-builder 1.4.9) with a throwaway user installation: build, install, launch on a headless X server, window shown, a second launch exits at once.
- Not checked: microphone, camera and screen sharing inside the sandbox, a live tray panel, aarch64.

## Submitting

Flathub asks for a pull request against `flathub/flathub` (branch `new-pr`) from the maintainer's own GitHub account, and the maintainer then owns the `flathub/com.amni_scient.Haven` repository. The submission text is in `issues/fixes/feat-desktop-packaging.md`. Flathub's policy on AI-written submissions asks that the submitter understands and can maintain what they send, so the submitter should read the manifest first.
