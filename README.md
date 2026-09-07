# Haven Desktop (Tauri)

**Private chat, reimagined for your desktop.**

This is a **Tauri 2** port of [ancsemi/Haven-Desktop](https://github.com/ancsemi/Haven-Desktop) (originally Electron). It connects to any [Haven](https://github.com/ancsemi/Haven) server with a lighter native shell: system tray, notifications, local server hosting, and per-app audio capture via the same WASAPI / PulseAudio native code.

> **You need a Haven server to use this app.** Haven Desktop is a client — it connects to a Haven server running on your (or a friend's) machine. Download and set up [Haven](https://github.com/ancsemi/Haven) first if you do not already have one.

## Features

| Feature | Notes |
|---|---|
| **Host or Join** | Detect/start a local Haven server, or connect to a remote URL |
| **System tray** | Minimize to tray, show/quit from tray menu |
| **Native notifications** | OS notifications for desktop alerts |
| **Per-app audio** | Native PulseAudio (Linux) / WASAPI (Windows) capture, wired through Tauri events |
| **Desktop prefs** | Language, minimize-to-tray, shortcuts, server history |
| **Injected bridge** | Haven web UI gets `window.havenDesktop` with the same surface as the Electron app |

## Prerequisites

- **Node.js** 18+
- **Rust** 1.85+ (stable) — install from https://rustup.rs
- **C++ toolchain**
  - **Windows:** Visual Studio Build Tools with the “Desktop development with C++” workload (MSVC)
  - **Linux:** `g++`, plus `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`, `libpulse-dev`, `pkg-config`
- A running or installable **Haven server** for Join / Host flows

## Develop

Run these from the **project root** (the folder that contains `package.json`), not your home directory.

### Windows (PowerShell or cmd)

```bat
cd path\to\haven-desktop
npm install
npm run tauri:dev
```

Do **not** prefix with `CXX=g++` — that is Linux/macOS shell syntax and will fail in `cmd.exe`.

### Linux / macOS

```bash
cd path/to/haven-desktop
npm install
CXX=g++ npm run tauri:dev
```

The welcome window opens on Vite port `14370`. Use **Join a Server** with your Haven URL, or **Host My Server** if you have a local Haven checkout.

## Build installers

**Windows:**

```bat
npm run tauri:build
```

**Linux:**

```bash
CXX=g++ npm run tauri:build
```

Artifacts land under `src-tauri/target/release/bundle/` (NSIS on Windows; `.deb` / AppImage on Linux).

## Architecture

```
├── index.html / src/          # Welcome UI (Vite)
├── src-tauri/
│   ├── src/                   # Rust shell (windows, tray, IPC, server manager)
│   ├── inject/app-bridge.js   # Injected into Haven webviews
│   └── native/                # C++ audio capture (PulseAudio / WASAPI) via C ABI
└── assets/                    # App icon
```

Electron `BrowserWindow` / `BrowserView` / `ipcMain` map to Tauri windows, webviews, and `#[tauri::command]` handlers. The Node N-API audio addon is replaced by a C ABI (`haven_audio_*`) linked from Rust.

## License

AGPL-3.0 — same as Haven / Haven Desktop.
