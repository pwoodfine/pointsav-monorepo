# local-orchestration-command — IaC for app-orchestration-command

Infrastructure-as-code for the Foundry workspace CommandCentre. Authored by
project-orchestration Totebox; Command Session installs to workspace-tier
`/srv/foundry/infrastructure/local-orchestration-command/`.

## Files

| File | Purpose |
|---|---|
| `local-orchestration-command.service` | systemd unit; binds 127.0.0.1:8020; service user `local-orchestration-command` |
| `bootstrap.sh` | Idempotent installer; copies binary + unit; creates service user; smoke tests `/healthz` + `/readyz` |
| `README.md` | This file |

## Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `COMMAND_BIND_ADDR` | `127.0.0.1:8020` | HTTP bind address (loopback; Phase 3) |
| `COMMAND_INSTANCE_ID` | `gateway-orchestration-command-1` | Stable instance label for WORM ledger |
| `COMMAND_PAIRINGS_PATH` | `/srv/foundry/pairings.yaml` | Cluster topology source (read-only at startup) |
| `COMMAND_CLONES_ROOT` | `/srv/foundry/clones` | Archive manifest + inbox root |
| `COMMAND_AUDIT_LEDGER_PATH` | `/var/lib/local-orchestration-command/audit.jsonl` | WORM append-only pairing ledger |
| `COMMAND_LICENSE_TOKEN` | (unset = observation mode) | Ed25519-signed license token |
| `COMMAND_LICENSE_PUBKEY_HEX` | (unset = observation mode) | License public key hex |
| `COMMAND_SLM_BINARY` | (unset = no child) | Path to app-orchestration-slm binary |
| `RUST_LOG` | `info` | tracing log level |

## Bootstrap procedure

1. Build the release binary from the project-orchestration cluster:
   ```bash
   cd /srv/foundry/clones/project-orchestration/pointsav-monorepo/app-orchestration-command
   CARGO_TARGET_DIR=/srv/foundry/cargo-target/orchestration-command \
     cargo build --release -p orchestration-command-server
   ```
2. Command Session runs `sudo bootstrap.sh` to install + start.
3. Smoke-test with curl to `/healthz` and `/readyz`.
4. Set license via `systemctl edit` drop-in (see bootstrap.sh §7).
5. Update cluster manifest tetrad deployment status to `active`.

## Observation mode

When `COMMAND_LICENSE_TOKEN` is absent, the server starts in observation mode:
- All read endpoints (`/healthz`, `/readyz`, `/v1/archives`, `/v1/audit/rollup`) work normally.
- Write endpoints (`/v1/invite`, `/v1/pair`) return HTTP 402.
- The running binary is never killed by license expiry (Doctrine §54).

## Port assignments

| Service | Port |
|---|---|
| `app-orchestration-command` | 127.0.0.1:8020 |
| `app-orchestration-graph` stub | 127.0.0.1:8021 |
