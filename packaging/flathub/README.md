# Haven Desktop on Flathub

Files for a Flathub submission of Haven Desktop (Tauri) under the ID `com.amni_scient.Haven`. Flathub's [Generative AI policy](https://docs.flathub.org/docs/for-app-authors/requirements#generative-ai-policy) does not allow AI-generated or AI-assisted content in the manifest, or AI-opened submission pull requests, so the manifest is written and submitted by hand and is not kept here.

## Files

- `com.amni_scient.Haven.desktop`, `com.amni_scient.Haven.metainfo.xml`: the desktop entry and AppStream metadata (both pass `desktop-file-validate` and `flatpak-builder-lint appstream`). AI-generated; disclose them in the submission.
- `flathub.json`: limits Flathub builds to x86_64 and aarch64.
- `cargo-sources.json`, `node-sources.json`: offline source lists generated with [flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools) from `src-tauri/Cargo.lock` and `package-lock.json`. Regenerate them whenever either lockfile changes.
- `screenshots/welcome.png`: the screenshot the metainfo points at on `main`.

## Each release

Add a `<release>` entry to the metainfo, and regenerate the source lists when a lockfile changed.

## App ID

`com.amni_scient.Haven` follows the domain `amni-scient.com` (a hyphen becomes an underscore). After acceptance, serve the token Flathub gives as `https://amni-scient.com/.well-known/org.flathub.VerifiedApps.txt`.
