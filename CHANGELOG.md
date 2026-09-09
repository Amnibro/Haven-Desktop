# Haven Desktop (Tauri) Changelog

Tracks the Electron app at [ancsemi/Haven-Desktop](https://github.com/ancsemi/Haven-Desktop). Each entry names the upstream release it is level with.

## Unreleased

### Fixed
- **Linux crashed on launch (SIGABRT) when no AppIndicator library was installed.** The tray was declared in `tauri.conf.json`, so Tauri built it before our setup ran, and `libappindicator-sys` panics when it cannot `dlopen` `libayatana-appindicator3` or `libappindicator3`; with `panic = "abort"` that killed the process before any window appeared. The AppImage and deb bundle the library, but a bare `haven-desktop` binary on a distro without it (Arch without `libayatana-appindicator`) never started. The tray is now created in code only after probing for the library; without it the app runs tray-less, logs one line saying so, and closing the window quits instead of hiding into a tray that isn't there. `desktop_get_prefs` reports `trayAvailable` so the settings UI can grey out minimize-to-tray.

### Added
- **`install-linux.sh`**: installs the AppImage (local file, `~/Downloads`, or the latest GitHub release) or a bare binary with `--bare`, writes the menu entry and icons, falls back to extracting the AppImage where FUSE 2 is missing, and prints the distro's install command for anything a bare binary is missing. `--uninstall` reverses it.

## v2.2.0-tauri (2026-09-08) - level with Electron 1.4.30 + Haven-Desktop PR #52

### Fixed
- **Mute, deafen and push-to-talk shortcuts did nothing.** The global shortcut handler was an empty closure. It now emits the same events the Electron preload sends (`voice:mute-toggle`, `voice:deafen-toggle`, `voice:ptt-toggle`, `voice:ptt-down` / `voice:ptt-up`), and because the shortcut plugin reports press and release, hold-mode push-to-talk is real here, with OS auto-repeat ignored while the key stays down. Dispencer2's "hold spam-toggles" cannot happen on this build.

### Notes
- Screen sharing uses WebView2's own source picker on Windows, which already lists Haven's window, scrolls and follows the system theme, so the three Electron picker fixes in Haven-Desktop PR #52 have no Tauri counterpart.
- The web-side forum, NSFW, settings search and create-form changes (ancsemi/Haven PR #5595) arrive through the server, nothing to port.

## v2.1.0-tauri (2026-09-08) - level with Electron 1.4.30

### Added
- **Linux AppImage and deb** built by CI on Ubuntu 22.04 next to the Windows installer, published on the same release with `latest.json`.
- **Signed auto-update.** The app checks the release feed once at startup and offers to install a newer build, then restarts.
- **Themed app icon and title bar**, the way the mobile launcher icon works: the theme's background behind the hexagon-plus-H glyph in its accent, on the window, the taskbar, the tray, and on Windows 11 the caption bar.
- **Unread dot** on the taskbar icon while anything is unread (Windows).
- **Lean while hidden.** Minimized or in the tray, WebView2 is asked to run at its low memory target: the process tree drops from about 795 MB to 170 MB resident.
- `target="_blank"` links and `window.open` work: same-server deep links open in place, other links in the system browser.

### Fixed
- **Self-signed Haven servers loaded as "Your connection isn't private".** Accepted on Windows through the WebView2 process flags and on Linux through the WebKitGTK TLS policy, matching the Electron app. macOS has no hook in wry and needs a trusted certificate or plain http.
- **Every incoming notification switched the channel.** The toast command emitted the click event on show.
- **Switching servers from the rail opened the system browser.** The bridge aborted before the page had a root, so the web app fell back to `window.open`. Servers in the history also pass the navigation guard now.
- The exe carried the Tauri default icon; the icon set is generated from the Haven artwork.
- A second launch focuses the running app; the main window remembers its size.

### Upstream 1.4.27 to 1.4.30
Already in this port: Portuguese (Brazil) localization with the Language menu, the UTF-8 BOM fix in server detection, clipboard write-text. Electron-only and skipped: BrowserView bounds on Wayland and first maximize, the Autofill switch, the electron-builder AppImage runtime (Tauri's linuxdeploy runtime is separate; Linux testers please report any FUSE error).

## v2.0.0-tauri (2026-09-07)
- Tauri 2 port of Haven Desktop 1.4.26: welcome screen, host or join, tray, notifications, per-app audio capture (WASAPI / PulseAudio), Windows NSIS installer.
