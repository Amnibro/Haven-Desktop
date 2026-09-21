# Haven-Desktop architecture map

Updated: 2026-07-31

## Role

Electron shell for Haven: welcome (host/join), multi-server BrowserViews, tray, audio capture, desktop notifications.

## Pairing (local monorepo)

| Path | Role |
|------|------|
| `../Haven-Braid` | Server + **Braid** chat UI (use this for Braid theme) |
| `../Haven` | Older/plain server tree (may lack `node_modules`) |
| `Haven-Desktop` | This app |

## Server detection (`src/main/server-manager.js`)

Order: saved `userPrefs.serverPath` → sibling **Haven-Braid** → sibling Haven → common install paths.

Prefers directories that have `node_modules/better-sqlite3`. If preferred port already answers `/api/health`, reuses it instead of killing the process.

Host spawn sets `FORCE_HTTP=true` unless overridden, so local URL is `http://localhost:<port>`.

## How to see Braid (stock flow)

1. `Haven-Braid\Start Haven.bat` — same launcher as upstream Haven
2. `Haven-Desktop\Start Haven Desktop.bat` or `Haven.exe` — Host (auto-finds Haven-Braid) or Join the printed URL
3. Theme picker: **Braid** / **Braid Light**; optional **▤** form

Remote OG does **not** ship Braid CSS. No custom join/start bats.
