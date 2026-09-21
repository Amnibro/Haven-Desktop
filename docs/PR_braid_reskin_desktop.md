# PR — Braid UI: restyle welcome + splash, fix clobbered disable-features switch

**Repo:** `ancsemi/Haven-Desktop` ← `braid-reskin`
**Base:** `main`

## Scope

The desktop app loads the chat UI from the server (`loadURL(<server>/app.html)`),
so the only surfaces this app owns are the welcome/server-picker and the splash.
Those are restyled here to match the Braid visual language used by the web
client — same tokens, quieter surfaces, one accent.

- `src/renderer/welcome.css` — token block + surface restyle
- `src/renderer/splash.html` — slate background, accent spinner
- `src/renderer/welcome.html` — one tagline reword

No behaviour changes: same IDs, same flows, same IPC.

## Bug fix included

`main.js` called `app.commandLine.appendSwitch('disable-features', …)` **twice** —
once to suppress Autofill CDP warnings, then again for the WGC-noise block.
Chromium stores switches in a map, so the second call **replaced** the first and
`AutofillServerCommunication` was never actually disabled.

Both now live in a single comma-joined list, with a comment so a third caller
doesn't silently reintroduce the same bug. This also matters for anyone adding a
WebRTC feature flag later — it has to go in that same list to take effect.

## Not included

Deliberately kept out of this PR so it stays a reskin: the custom-activity work
(process-list IPC in `main.js`, `app-preload.js` bridge) and its server-side
counterpart in the web repo.

## Build note

`npm run build:win` currently fails at `uiohook-napi` if the toolchain is newer
than node-gyp expects — node-gyp probes for VS 2013–2022 and gives up on VS 2026.
`uiohook-napi` ships a `win32-x64` prebuild, so `electron-builder --win
-c.npmRebuild=false` packages fine. Unrelated to this change, but worth knowing
if CI picks it up.
