# Haven Desktop (Tauri) Changelog

Tracks the Electron app at [ancsemi/Haven-Desktop](https://github.com/ancsemi/Haven-Desktop). Each entry names the upstream release it is level with.

## v2.4.0-tauri (2026-09-10)

### Fixed
- **Connection-error buttons actually leave the dead server.** Welcome stays
  hidden instead of being destroyed, so Go Back to Welcome and Go Back to My
  Server can show it again. The page uses in-app actions instead of a dead
  WebView2 document.

## v2.3.0-tauri (2026-09-10)

### Added
- **Unreachable server is a Haven page now.** If localhost (or any server) refuses
  the connection, the main window leaves Edge's "can't reach this page" and shows
  Try Again, Go Back to Welcome, and Go Back to My Server when there is one.
  Those buttons keep Welcome hidden (not destroyed) so they can bring it back.
- **Remembered-server launch no longer flashes Host / Join.** That window stays
  hidden when skip-welcome is on. Closing the main window quits the app (unless
  Minimize to tray is on), so the tray does not keep a dead Edge page around.
- **Save Image uses a native save dialog.** WebView2 ignores `<a download>` on
  a lot of http(s) media, so the bridge takes the image bytes and opens the
  OS picker. Pair with a Haven server that calls `havenDesktop.saveImage`.
- Forum feed and gallery tile sliders live on the server (Haven PR #5630).
  Connect to a server that has that build and they show up here too.

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
