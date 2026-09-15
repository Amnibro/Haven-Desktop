# Amni Relay

Enterprise communications client for **Amni Workspace**.

This is a **separate product package**, not a branding flag on Haven Desktop. It vendors Haven Desktop core and connects to a Haven-compatible engine for channels, DMs, files, roles, and WebRTC.

Haven remains unpaid. Relay does not add Stripe, entitlements, or licensing telemetry.

Read [PRODUCT.md](PRODUCT.md) for Workspace vs Relay vs Haven engine.

## Layout

```
amni-relay/
├── PRODUCT.md              # Product definition
├── VENDOR.md               # How Relay vendors Haven Desktop
├── src/
│   ├── identity.js         # Amni Relay / Amni Workspace identity
│   ├── vendor-haven.js     # Resolves sibling Haven Desktop sources
│   ├── main.js             # Electron entry (Amni chrome + engine session)
│   ├── sso/                # OIDC + SAML stubs
│   ├── enterprise/         # Group/DM policy + Office/Workspace connectors
│   ├── theme/              # Dark-engineer tokens + overlay CSS
│   └── renderer/           # Welcome, splash, about
└── test/                   # Node tests (no Electron required)
```

## Develop

```bash
# from repo root
npm install                 # Haven Desktop / Electron
cd amni-relay
npm test
../node_modules/.bin/electron .
```

SSO and connector flows are stubs. Completing an SSO login creates a local session only.

## Packaging

`electron-builder.yml` is the Amni Relay installer identity (`com.amni.relay`, product name **Amni Relay**). It does not change Haven’s `com.haven.desktop` packaging.
