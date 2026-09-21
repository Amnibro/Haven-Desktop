# Haven-Desktop (local fork)

Local fork of [ancsemi/Haven-Desktop](https://github.com/ancsemi/Haven-Desktop) restyled to match **Braid** and pair with `../Haven-Braid`.

## Intent

- Welcome / splash / titlebar use the same Braid visual language as the chat UI.
- Electron features (per-app audio, tray, notifications, host/join) stay intact.
- Merge to upstream later if desired; local `braid-ui` branch is the work branch.

## Run (same as ANCSemi Haven — stock launchers only)

**Server:**

```bat
Haven-Braid\Start Haven.bat
```

**Desktop (dev):**

```bat
Haven-Desktop\Start Haven Desktop.bat
```

**Desktop (installed):** `Haven.exe` or the Setup installer under `dist\`.

In Desktop: **Host a server** (auto-finds sibling `Haven-Braid` when deps are present) or **Join** the URL `Start Haven.bat` prints.

Theme picker → **Braid** / **Braid Light** (▤ = form). Remote OG does not ship Braid CSS.

Under the hood: detect prefers Haven-Braid; corrupt/BOM `config.json` is repaired so main process does not crash; host reuses a live `/api/health` on the preferred port.
