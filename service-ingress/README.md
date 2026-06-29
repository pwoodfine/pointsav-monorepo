# service-ingress

mTLS Phase A public ingress for `os-console`. A TLS-terminating reverse proxy that
exposes localhost-only services over HTTPS/TLS 1.3 so `os-console` can connect from
a remote laptop without requiring port 2222.

## What it does

Listens on `0.0.0.0:8443`. On first start, auto-generates a self-signed CA and server
certificate under `~/.config/service-ingress/` and prints the SHA-256 fingerprint.
`os-console` pins this fingerprint (TOFU auto-pin to `~/.config/os-console/server-hostkey`);
subsequent connections verify against the pinned value, rejecting any MITM.

Path routing (all upstream targets are localhost-only):

| Path prefix | Upstream |
|---|---|
| `/v1/proof/*` | `http://127.0.0.1:9092` (service-content) |
| `/v1/content/*` | `http://127.0.0.1:9092` (service-content) |
| `/v1/search/*` | `http://127.0.0.1:9092` (service-content) |
| `/doorman/*` | `http://127.0.0.1:9080` (Doorman) |
| `/health` | `200 OK {"status":"ok"}` |

## Configuration

Optional config file at `~/.config/service-ingress/config.toml`:

```toml
[listen]
port = 8443

[upstream]
content = "http://127.0.0.1:9092"
doorman = "http://127.0.0.1:9080"
```

## Build and run

```bash
cargo build --release -p service-ingress
./target/release/service-ingress
```

## Role in the os-console architecture

`service-ingress` is the Phase A transport layer. It allows `os-console` to connect to
a remote Totebox Archive over a hotel or corporate network (outbound 443/8443 — no
inbound firewall rules on the laptop). GCE port 2222 remains closed. The mTLS binding
(MBA pairing ceremony) is the application-layer door; `service-ingress` is the pipe.

Planned Phase B: WireGuard PPN underlay for NAT/CGNAT environments.

## License

AGPL-3.0-or-later. See `LICENSE` at the repository root.
