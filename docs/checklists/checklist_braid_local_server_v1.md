# Checklist — See Braid theme (local server) v1

Date: 2026-07-31

- [x] Diagnose: Desktop was joining remote `anchaven.duckdns.org` (OG, no Braid UI)
- [x] Diagnose: `detectServer()` preferred sibling `Haven` (no `node_modules`) over `Haven-Braid`
- [x] Diagnose: Desktop host mode always `taskkill`s port 3000 (kills healthy Braid server)
- [x] Verify Haven-Braid serves `/css/braid-shell.css`, `/css/braid-form.css`, themes `braid` / `braid-light`
- [x] Prefer `Haven-Braid` in Desktop `detectServer()`; require `better-sqlite3` when possible
- [x] Reuse existing healthy server on preferred port instead of killing it
- [x] Desktop host spawn uses `FORCE_HTTP=true` by default for stable local HTTP
- [x] Prefer stock flow only: `Start Haven.bat` + `Start Haven Desktop.bat` / `Haven.exe`
- [x] Remove custom Join/Start Braid bats (user feedback)
- [x] Repair corrupt/BOM `config.json` that crashed main process
- [x] Point host `serverPath` at Haven-Braid; strip BOM prefs
- [x] Backup `server-manager.js` / `main.js` → `backups/v_braid_detect/`
- [x] Update CHANGELOG / architecture_map / FORK notes
- [x] Verified launch: host reuses `http://localhost:3000`, main window **Haven - Login**, loads local `/js/*` (no SSL error)
- [ ] User confirms Braid theme selected in UI (Braid / Braid Light + optional ▤)
