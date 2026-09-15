# Amni Relay

**Amni Relay** is a distinct product in the **Amni Workspace** suite. It is not a theme toggle on Haven Desktop.

## Naming

| Name | What it is |
|---|---|
| **Amni Workspace** | The suite. Productivity and communications under the Amni banner. |
| **Amni Relay** | The communications product face (Teams-line replacement). This package. |
| **Haven** | The unpaid communications **engine**. Channels, group messaging, DMs, files, roles, WebRTC voice/video/screen. |

Relay skins and packages the client as Amni. It vendors Haven Desktop core functions. It does not rebadge Haven in place and it does not change Haven’s unpaid / no-billing nature.

## What this package is

In-repo product surface (`amni-relay/`) that:

- Ships its own app identity, chrome, splash, about dialog, and color tokens
- Vendors Haven Desktop modules in-place (`../src`) for server host, per-app audio, and the existing BrowserView preload
- Adds enterprise **stubs**: SSO (OIDC / SAML), group+DM hardening policy, Microsoft Office and Google Workspace connectors (migrate + open docs)
- Injects an Amni overlay into the Haven engine web app after connect

Long-term home can be a standalone repo (suggested: `Amnibro/Amni-Relay`) with this tree as the starting surface — submodule or vendor Haven Desktop instead of living beside it. This Haven-Desktop tree is the wrong vehicle for a theming-only PR; it is an acceptable **scaffold host** until that split.

## What this package is not

- Not a Microsoft 365 clone
- Not a Haven billing layer
- Not Stripe, entitlements, or licensing telemetry — on Relay or on Haven
- Not a `AMNI_RELAY_BRANDING=1` flag inside Haven

## Haven unpaid constraint

Haven upstream stays unpaid and unentitled.

- Do not add Stripe to Haven
- Do not add license checks or entitlement gates to Haven
- Do not add licensing telemetry to Haven
- Relay may grow enterprise **controls** (SSO, policy, connectors) without turning Haven into a paid SKU

## Foundation (from Haven)

Relay’s session still talks to a Haven-compatible engine for:

- Channels and group messaging
- Direct messages
- Files
- Roles
- WebRTC voice, video, and screen share

Desktop extras (local server detect/start, WASAPI / Pulse per-app audio) are reused via `src/vendor-haven.js`.

## Enterprise scaffold

| Area | Status | Notes |
|---|---|---|
| OIDC | Stub | Authorization URL + local stub session |
| SAML 2.0 | Stub | AuthnRequest XML + local stub session |
| Group + DM hardening | Policy module | Auth-required DMs, admin-only invites, external sharing off by default |
| Microsoft Office | Connector stub | `open-docs`, `migrate-mail` |
| Google Workspace | Connector stub | `open-docs`, `migrate-mail` |

Stubs are intentional. They define the product boundary without pretending IdP or Graph/Workspace export is finished.

## Run

From this directory, with Electron available from the parent Haven Desktop install:

```bash
npm test
# after `npm install` in the repo root:
../node_modules/.bin/electron .
```

See [README.md](README.md).
