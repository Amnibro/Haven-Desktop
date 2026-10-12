# Haven Desktop (Tauri) Changelog

Tracks the Electron app at [ancsemi/Haven-Desktop](https://github.com/ancsemi/Haven-Desktop). Each entry names the upstream release it is level with.

## v2.6.1-tauri (2026-10-12)

Works on VPNs, guest Wi-Fi, trusted networks and mobile hotspots without confusing questions. Level with Electron 1.6.2.

- **Servers that send only their own certificate connect without a question on Linux.** Some servers (haven.moviethingy.xyz among them) leave out the Let's Encrypt intermediate, and Linux has no way to fetch it, so the app asked whether to trust a perfectly valid certificate. The common public intermediates (Let's Encrypt R10–R14, E5–E9, YE/YR and their roots, Google Trust Services, Sectigo/ZeroSSL, DigiCert) now ship with the app as chain material, checked against your system's trusted roots. Anything else is fetched from the certificate's issuer link once and cached.
- **Your system's trusted certificates come first.** A company or VPN root you installed on the computer is trusted with no question.
- **One question, worded for what is happening.** When a VPN or network presents its own certificate for your server, the question says so ("Your VPN is presenting its own certificate", or "This network…"), names who issued it, suggests allowing the server in the VPN's web filter, and keeps the fingerprint and dates under Details. Servers on your own network or reached by IP keep the self-hosted wording. The question appears once per server and certificate, however many requests hit it.
- **Trusting a second certificate keeps the first.** Each server remembers every certificate you trusted, so switching between your VPN and your home network does not ask again. A certificate different from the ones you trusted gets the same single question saying so, instead of a dead end.
- **No questions about other sites.** When a network sends Haven somewhere else (a hotel or guest Wi-Fi sign-in page, a firewall's block page), the app no longer asks about that site's certificate or bounces it to the browser silently: it says "Your network or VPN redirected Haven to …" with **Open sign-in page** (in your browser) and **Try Again**.
- **Guest Wi-Fi sign-in.** When Linux reports a sign-in portal, the app shows "Sign in to this Wi-Fi to continue" and reconnects by itself once you are signed in.
- **Quiet reconnects.** A page load that fails during a network change is retried quietly before any error shows, and the connection page tries again by itself when the server can be reached again. On Linux, switching networks or turning a VPN on or off re-checks the server's certificate in the background so the chat reconnects without a reload. Windows and macOS still accept every certificate, as before.

## v2.6.0-tauri (2026-10-12)

- **New update signing key.** 2.6.0 is signed with a new updater key, so copies of 2.5.x installed from the AppImage or the Windows installer cannot update to it on their own: download 2.6.0 once and later versions update in-app again. Flatpak and Debian/Ubuntu users update through their package manager.
- **Linux packages from amni-scient.com.** A Flatpak remote (`flatpak install --from https://downloads.amni-scient.com/flatpak/haven.flatpakref`) and a signed APT repository (`curl -fsSL https://downloads.amni-scient.com/apt/install.sh | sh`) keep Haven updated with the rest of the system. Releases are built locally with `tools/release-linux.sh`.

Level with Electron 1.6.2.

- **One server list for every server.** The app keeps a single list that every server's sidebar follows (Haven 4.19.0 and later). A server you remove stays removed instead of coming back from another server's copy, opening it or adding it again brings it back, and the order you drag servers into and the names you give them show on every server, the newer rename winning. The list holds 100 servers instead of 20 and, when full, drops the one used longest ago. A name you type when adding a server is kept as your own, and a saved order can only move servers already in the list. A damaged saved list no longer stops servers from opening.
- **Server pages can no longer change your server list without asking you.** Adding, removing, renaming or changing the icon of another server, or opening a server you do not have, shows the app's own question naming the change and the server that asked. Reordering and a server reporting its own name need no question. A no is not asked again until you click to make that change again, and you can ignore a server's requests until Haven restarts. Addresses such as `http://nas:3000` open and stay in the list.
- **Server pages get only what Haven needs.** A server page could change the app's own settings (including the folder Host My Server starts from), start the local server, read the settings file directly, open windows, register global shortcuts, read the clipboard through the plugin, list the apps playing sound and start capturing one, and quit the app. Those are now reserved for the app's own screens. A server name containing HTML no longer runs as code on the Welcome and connection-error pages.
- **Server certificates are checked on Linux.** The app accepted every certificate from every host, so someone on the same network could pose as a remote server. A certificate your system trusts works as before, this computer and your local network stay automatic, and a remote server with a certificate of its own asks once whether to trust it and remembers the fingerprint; if that certificate later changes, the app says so and asks again. Servers you had already used are trusted on their next connection without asking. Windows and macOS still accept every certificate for now.
- **Joining a server by its public IP address** asks the usual certificate question instead of failing, and an `http://` address on the server's HTTPS port is opened as `https://`.
- **The update banner is back and reliable.** A found update shows as a banner on every server page (Update Now, then Restart Now) instead of a one-off dialog at launch, closing it keeps it closed until a newer version turns up, and the app checks again every six hours while it stays open. **Check for Updates** on the tray menu checks on demand and says when you are already on the latest version.
- **Push to talk does nothing outside a voice call.** The key no longer plays the mute sound or flips your saved mute state when you are not in voice. It reads the mic state from Haven itself, follows the key inside the focused window too (needed on Wayland, where the global shortcut sees nothing while Haven is focused), and a toggle counts once per press.
- **A slow connection no longer ends in "Connection Problem".** The app gave up on a server that had not answered within 15 seconds; it now keeps waiting while the page is still loading, up to 90 seconds, and never while a certificate question is open. On Linux, a load the app cancels itself (a retry, a redirect) no longer counts as a failure.
- **A server shows as soon as its page is ready**, the way a browser does, instead of holding the loading screen until every picture on it has arrived.
- **The taskbar flashes when a notification shows** and stops when you click into the app. Returning to the window hands keyboard focus back to the page on Linux.
- **Welcome and connection screens look like the rest of Haven.** The welcome window has an edge and the Amni OS title-bar buttons, the language menu and checkbox follow the theme instead of showing as white boxes, and the welcome, join and connection-error screens take the colors (light or dark) of the last server you used. The connection-error page lost its duplicate buttons and the gap under them, and Go Back to My Server no longer offers the server that just failed. The Desktop App settings hide switches that did nothing in this build (hide menu bar, force SDR, GPU vsync, frame-rate cap).
- Haven's website is haven-app.com.
- Not ported, Electron-only: X11 mode, the VA-API switch, the HDR colour profile, the Electron screen-share picker, hardware encoder preference and per-app audio in the picker (the webview's own share picker is used), and the hidden menu bar strip.

## v2.5.6-tauri (2026-10-11)

- **macOS.** A universal (Apple Silicon and Intel) `.dmg` is now published with every release. Self-signed Haven servers are accepted through a WKWebView certificate hook, unread messages put a badge on the Dock icon, the tray icon lives in the menu bar, and a server started from the app finds Node installed by Homebrew, nvm, fnm or asdf even when Haven was launched from Finder. System audio capture for screen share is not available on macOS yet.
- **Flatpak.** Linux releases include a `.flatpak` bundle on the GNOME runtime with tray, notifications, microphone and camera. It does not update itself; hosting a server from it needs Node on the host, so use the AppImage or deb for that.
- **Joining and creating channels.** The Join button and Enter key now accept a full invite link as well as a bare code, and Create sends the channel options (private, temporary, forum, add everyone, template) through the live connection. A join you asked for switches to the new channel; joins Haven repeats on reconnect are left alone.
- **Connecting from Welcome** no longer waits on a separate `/api/health` request first, which failed against servers that do not answer it and blocked the connection.

## v2.5.5-tauri (2026-10-05)

- **Pop-up windows open.** Linking Steam or Spotify under Connections, the single sign-on consent page and game pop-outs called `window.open`, which a Tauri webview silently refused, so Haven reported "allow pop-ups" and nothing happened. Pop-ups now open in their own Haven window, close themselves when the page finishes, and have no access to the desktop bridge.
- **No false "Connection Problem" page.** A load that was superseded (Haven sending a signed-out `/app` to the login page, a blocked external link, a download) was treated as the server being down.
- **Microphone, camera and screen share on Linux.** WebKitGTK refuses media and device-list requests that nobody answers, so voice, webcam, screen share and named input devices failed. The app window now grants them (location stays denied).
- **Downloads on Linux.** Backups, audit logs and attachments save to the Downloads folder under a non-clashing name, with a notification saying where. Attachment links marked `download` are no longer turned into pop-ups.
- **Clicking a notification on Linux** brings Haven forward and opens the channel the message came from.
- **Start on login** really registers the app with the system, and **Start hidden** keeps the window in the tray on that launch.
- **OpenID sign-in** stays inside the app instead of bouncing the identity provider to the system browser.
- **Pasted file paths** are only read when that file is actually on the clipboard, so a server page can no longer ask the app to read arbitrary local files.
- Real STUN connectivity warnings are shown again when the webview has WebRTC (Windows, Amni OS); only WebRTC-less WebKit builds hide them.
- Shortcuts the system cannot register say so instead of reporting a conflict, the window size is saved without a disk write on every resize step, and returning to Welcome without an open server no longer leaves the next close stuck.

## v2.5.4-tauri (2026-09-29)

- **Notification banners for the channel you have open.** Haven only shows a banner for the open channel when the page is hidden, and WebKitGTK keeps the page "visible" while the window sits unfocused behind other windows, so those messages played the sound with no banner, on every server. The bridge now shows the banner itself when the window lacks OS focus (mentions, replies and DMs as usual, mutes respected), and stands down on servers whose own client already accounts for focus.
- **File uploads (zip, tar, pdf, etc.) via drag & drop and paste on Linux.** Disables Tauri's native drag-drop interception on webview windows so WebKitGTK's HTML5 drag-and-drop reaches the chat composer and message area. Also adds native `clipboard_read_files` command reading Linux clipboard file targets (`text/uri-list`, Nautilus/GNOME copied files) via Wayland (`wl-paste`) and X11 (`xclip`) when copying files in Linux file managers (Nautilus, Dolphin, Thunar) and pasting with Ctrl+V, reading the file data on the desktop host and cleanly queuing it into the composer attachment queue.
- **Fixed false "Couldn't paste the picture" toast on file paste.** Because WebKitGTK blocks DOM DataTransfer access to `text/uri-list` on paste events, pasting non-plain-text content like a copied `.zip` file previously fell through to `clipboard_read_image` and triggered a misleading error toast. The bridge now checks native clipboard files first and suppresses spurious image error toasts when clipboard content is not an image.
- **Suppress false STUN/TURN connectivity warnings.** When WebKitGTK is built without WebRTC (`-DENABLE_WEB_RTC=OFF`), STUN probes throw `ReferenceError` on `RTCPeerConnection`, falsely claiming all STUN servers are unreachable. The desktop bridge and web client now check if WebRTC is supported before probing and suppress false STUN unreachable warning toasts across all servers (including synchronous pre-paint suppression on older remote servers like `haven.moviethingy.xyz`).

## v2.5.3-tauri (2026-09-24)

- **Pictures copied from a browser paste on Linux.** A picture copied from Chrome, Amni Browse or a web page ("Copy image") puts an HTML `<img>` next to the image. WebKitGTK exposes only the HTML, and the paste rescue stood down whenever any text type was present, so nothing pasted. It now stands down only for real plain text, stops the stray HTML paste, and reads the picture instead.
- A failed rescue shows an error toast instead of failing silently, and a rescue that never answers releases after 10 s so later pastes still work.

## v2.5.2-tauri (2026-09-24)

- **Pasting a picture works on Linux again.** The WebKitGTK paste rescue read the clipboard image fine, then turned the PNG data URL into a blob with `fetch()`. Haven's CSP (`connect-src 'self' ws: wss: https:`) has no `data:`, so the fetch was blocked and the empty `.catch()` hid it: Ctrl+V and right-click → Paste did nothing. The bridge now decodes the base64 directly, and logs rescue failures to the console instead of swallowing them.

## v2.5.1-tauri (2026-09-22)

Level with Electron 1.4.30.

### Fixed
- **Removing a server froze the app until it was killed.** Older Haven servers
  ask with the page's `confirm()`. The dialog commands were synchronous, so they
  ran on the GTK main thread, and `blocking_show()` waited there for a dialog
  that needed that same thread. The dialog and file-picker commands are async
  now. Any `alert`, `confirm` or `prompt` froze the app the same way.
- **"Are you sure?" never waited for an answer.** The bridge and
  tauri-plugin-dialog both replaced `confirm()` with an async version, so it
  returned a Promise and `if (!confirm(...)) return;` carried on regardless:
  deleting users, restoring backups, removing servers. Outside macOS the bridge
  puts the page's own `alert`/`confirm`/`prompt` back, and on Linux the app
  answers WebKit's `script-dialog` signal with a GTK dialog, so `confirm()`
  returns true or false and `prompt()` returns what was typed.
- **A theme colour could crash the app.** `#aaé` sliced a UTF-8 character in
  half and, with `panic = "abort"`, killed the process. Non-hex colours are
  ignored.
- **The connection-error page was blank on Linux.** It was served from
  `http://tauri.localhost`, which only exists on Windows; Linux and macOS use
  `tauri://localhost`.
- **Switching servers and Retry froze the window.** The server reachability
  check (up to 1.5 s per address) ran on the main thread. The server-switch
  commands are async and Retry and "Go Back to My Server" run on a worker.
- **Pasting a picture on Linux was still blocked.** `clipboard_read_image` was
  missing from the command permissions, so the 2.5.0 paste fix never ran.
- **"Copy image" copied base64 text.** It writes a real image now.
- **Two tray icons.** The config and `tray.rs` each made one; the config one
  is gone, so theme colours land on the icon with the menu.
- **A removed server came back.** Removing it now clears it as the primary
  server, the startup server and its badge.
- **Starting a local server could kill other programs.** It ran `kill -9` on
  everything `lsof` matched on port 3000, including clients of a remote :3000.
  That is removed (a free port is picked anyway) and the start wait no longer
  blocks the window.
- **Mute, deafen and push-to-talk keys did nothing after a restart** until the
  shortcut settings were opened. They are registered at startup.
- **A bad remembered server left only a tray icon.** Startup falls back to the
  Welcome window.
- **Welcome window buttons match Amni OS.** Minimize and close use the same
  line glyphs, spacing and hover tile as the system title bar and Amni-Browse
  (close turns the theme red), instead of Haven-purple text glyphs.

## v2.5.0-tauri (2026-09-20)

Level with Electron 1.4.30. Linux (Amni OS) only; Windows behaviour is unchanged.

### Fixed
- **Voice on Linux: WebRTC is switched on in WebKitGTK.** WebKit ships with
  `enable-webrtc` and `enable-media-stream` off and wry never sets them, so the
  page had no `RTCPeerConnection` and Haven reported every STUN server dead.
  The main window now enables both. This needs a WebKitGTK built with
  `ENABLE_WEB_RTC`, which Amni OS provides through its own webkit2gtk-4.1
  package; on a stock distro package the setting has nothing to enable.
- **Pasting a picture on Linux.** WebKitGTK gives the paste event an empty
  DataTransfer for a clipboard bitmap (WebKit bug 218519). The bridge reads the
  image through the clipboard plugin and replays the paste with a real PNG
  file, so the composer queues it the same way it does on Windows.

## v2.4.1-tauri (2026-09-15)

### Fixed
- **Phone notifications arrive while the desktop app is open.** A Haven server
  skips mobile push for anyone who has a socket reporting visible, and WebView2
  kept reporting visible whenever the window sat behind other windows or in the
  tray. The bridge now folds the OS window focus into every `visibility-change`
  the page sends, and the Rust side pushes focus changes and the tray hide into
  the page, so an unfocused desktop lets FCM and web push reach the phone.

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
