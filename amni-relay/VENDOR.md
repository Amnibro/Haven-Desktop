# Vendoring Haven Desktop

Amni Relay does not fork Haven’s `src/main/main.js` and does not add a branding flag to Haven.

`src/vendor-haven.js` resolves the sibling Haven Desktop tree:

| Relay use | Haven module |
|---|---|
| Host a local engine | `src/main/server-manager.js` |
| Per-app audio | `src/main/audio-capture.js` |
| Screen-share / desktop preload | `src/main/app-preload.js` |

Override the root with `AMNI_HAVEN_ROOT` when Relay lives in another repo (submodule or vendor copy).

Suggested standalone split: `Amnibro/Amni-Relay`, with Haven Desktop as a submodule. This in-repo package is the scaffold until that repo exists.

Haven’s own `package.json` / `electron-builder.yml` stay Haven (`com.haven.desktop`). Relay packaging is `com.amni.relay`.
