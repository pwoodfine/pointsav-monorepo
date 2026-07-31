# OS PrivateGit

<div align="center">

[ 🇪🇸 Leer este documento en Español ](./README.es.md)

</div>

### *Software Distribution OS — Full-Stack Installer*

**Platform:** x86_64-unknown-linux-gnu
**Status:** Engineering scaffold (pending engineering cycle)

## Architectural Mandate

`os-privategit` generates the bootable OS image that deploys the PointSav software
distribution stack on customer-owned hardware. It installs and operates three cooperating
services:

| Service | Port | Role |
|---|---|---|
| `app-privategit-marketplace` | 9202 | Storefront — product catalog, payment verification, license issuance |
| `app-privategit-source` | 9201 | Binary release server — authenticated downloads, Ed25519 token verification |
| `tool-wallet` | — | Polygon USDC payment watcher — receipt writer, HD address derivation |

This is the primary customer-facing distributable. Small business operators deploy
`os-privategit` to self-host the full `software.pointsav.com` stack on hardware they
control, without routing source code or payment flows through a third-party provider.

**Licensing Authority:** `os-privategit` holds the Ed25519 license registry and wallet
seed path. Both are provisioned by the operator at first boot and never leave the host
machine.
