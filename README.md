# TrueNAS SCALE Rust Prometheus exporter

**Release:** Steel Ferret — version 0.8.2.

A standalone, remote, read-only exporter targeting TrueNAS SCALE 25.04.2.x. One Rust executable serves cached pool measurements and exporter operational metrics for one or multiple configured TrueNAS targets. It runs without installing anything on TrueNAS.

**Development status:** Pool collection and opt-in system/CPU/memory/network/ARC collection are implemented. Authorized native and local Docker preflights and sanitized captured-response replay establish selected TrueNAS 25.04.2.6 compatibility. Finite synthetic integrated/repeated-recovery and separate-cgroup profiles pass. Independent live numerical/combined-daemon acceptance and production readiness remain open. Pool scans expose reported errors, progress and state. Disks/services/alerts remain planned. The existing Graphite pipeline remains independent.

Start with the [current project status](docs/project-status.md), [remaining-work plan](docs/implementation-plan.md), [conversation handoff](docs/handoffs/2026-11-14-handoff.md) and [restart prompt](docs/handoffs/2026-11-14-restart-prompt.md). Dated evidence records preserve their original scope.

## Build, test and run (Debian-based Linux)

Run from the repository root. The final process is explicitly offline and binds loopback; it makes no TrueNAS connections. Rust dependencies are locked. MSRV is 1.88; container tooling is separately required for Docker checks.

```sh
sudo apt-get update
sudo apt-get install -y build-essential ca-certificates curl pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/truenas-rustup.sh
sh /tmp/truenas-rustup.sh -y --profile minimal --default-toolchain 1.88.0
. "$HOME/.cargo/env"
rustup component add rustfmt clippy
cargo build --locked --manifest-path application/Cargo.toml
cargo fmt --manifest-path application/Cargo.toml --check
cargo clippy --locked --manifest-path application/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path application/Cargo.toml
# Keep running in this terminal; inspect http://127.0.0.1:9110/metrics locally.
EXPORTER_MOCK=true EXPORTER_LISTEN=127.0.0.1:9110 cargo run --locked --manifest-path application/Cargo.toml
```

For offline development after dependencies have been fetched, add `--offline` to Cargo build/test commands. `cargo run --manifest-path application/Cargo.toml -- healthcheck` checks the local process without loading credentials. Ctrl-C or SIGTERM stops all background work.

## Endpoints and behavior

- `/metrics`: OpenMetrics text with HELP/TYPE, exporter status and unexpired measurements; never queries TrueNAS. Overlapping scrapes during encoding receive 503 with an encoder-busy response.
- `/healthz`: process liveness; container healthcheck target.
- `/readyz`: 200 while every configured target has all enabled required snapshots fresh and any enabled compatibility gate accepted, otherwise 503. Pool-only mode requires pools; system mode adds system; realtime mode adds CPU/memory/network/ARC. A failed attempt can leave readiness 200 during the finite cache grace period.

Mock mode is labeled `truenas_exporter_mock_mode=1` and keeps `truenas_api_connected=0`. It publishes fictional data to exercise the process, not to simulate an authenticated NAS.

Normal mode requires either a validated TRUENAS_TARGETS_FILE or the legacy TRUENAS_WS_URL, TRUENAS_USERNAME and TRUENAS_API_KEY_FILE settings; WSS verification cannot be disabled. Do not run normal mode against MEMORY ALPHA without explicit operator authorization. Use docs/configuration.md for defaults and validation, not live credentials in shell history.

## Containers

With Docker installed: `docker compose -f deployment/compose.yaml config`, then `docker compose -f deployment/compose.yaml up --build`. The standalone file always uses mock mode, publishes no host port and builds with repository-root context. Its built-in liveness command is available through `docker compose -f deployment/compose.yaml exec truenas-exporter truenas-exporter healthcheck`.

For an independent image build use `docker build --platform linux/amd64 -f deployment/Dockerfile -t local/truenas-rust-exporter:review .`. Both base images are digest-pinned. The runtime uses a digest-pinned distroless cc-debian13 base with no shell or package manager. Recorded base-signature verification and security scan results are dated in docs/security-validation.md; refresh gates for a final candidate rather than treating old counts as current. No production files are changed by these examples.

## File inventory

| Path | Responsibility |
| --- | --- |
| TrueNAS_Codex_Handoff.md | Original brief and operating boundaries; current status/contracts are in docs/ |
| AGENTS.md | Concise repository-wide development instructions |
| application/Cargo.toml, Cargo.lock | One package, dependencies and reproducible resolution |
| application/src/main.rs | Process lifecycle, signals and local healthcheck command |
| application/src/config.rs | Typed settings, secret-file input and trusted TLS configuration |
| application/src/api/protocol.rs | Closed method allowlist, JSON-RPC and authentication decoding |
| application/src/api/client.rs | Bounded connection actor, request correlation and reconnect |
| application/src/collectors/pools.rs, system.rs, realtime.rs | Typed selected-field validation and explicit synthetic data |
| application/src/api/realtime.rs | Separate authenticated event stream and owned subscription lifecycle |
| application/src/state.rs | Independent typed group snapshots, monotonic expiry and metric encoding |
| application/src/scheduler.rs | Periodic reads without overlapping collection attempts |
| application/src/http.rs | Cached metrics, liveness and readiness routes |
| fixtures/truenas-25.04.2.6/ | Captured sanitized pool/system/realtime evidence with provenance |
| application/tests/ | Synthetic fixture, protocol, state, HTTP and loopback TLS tests |
| deployment/ | Dockerfile, offline Compose, integration/scrape examples and test-only runtime probe |
| docs/ | Architecture, protocol, compatibility, metrics, configuration and operations contracts |
| scripts/check.sh | Local Rust and Python quality gates |
| scripts/profile-isolated-cgroups.py | Bounded synthetic two-container resource controller |
| scripts/check-container.py | Local-daemon-only container runtime and security checks |
| .github/workflows/ci.yaml | Secret-free Rust checks and Docker build if hosted on GitHub |
| dashboards/ | Reserved for future review artifacts; no live imports |
| LICENSE | MIT license and project copyright |
| CHANGELOG.md | Development changes |

## Prioritized remaining work

1. Complete independent numerical/source-chart verification, especially ARC hit-ratio semantics, and separately scoped combined live-daemon/minimum-role acceptance.
2. Resolve intermittent process/configuration test failures, run hosted final-candidate CI and refresh relevant security/hardening evidence.
3. Prepare reviewed monitoring artifacts and an operator-approved Linux rollout with parallel Graphite/rollback; production-representative soaks can refine finite resource confidence.
4. Expand P1 disks/services/alerts and later P2 applications/tasks/VMs after P0 contracts and acceptance. See [implementation-plan.md](docs/implementation-plan.md) for exit criteria.

Licensed under the [MIT License](LICENSE). See docs/implementation-plan.md and docs/validation.md for actual validation and remaining acceptance gates.

For repeatable local container checks: `python3 scripts/check-container.py --context desktop-linux`. This uses the existing validation image, exports a test-only probe automatically, and removes its ephemeral test containers. Python 3 and OpenSSL are required on the host. No probe or diagnostic shell is shipped in the runtime. CI uses the same script against its local default context. Docker Desktop is installed on the development Mac; no remote Docker deployment is needed for these checks.

Distroless troubleshooting uses exporter logs, metrics and the built-in healthcheck. `docker exec ... sh` is unavailable by design. Preserve UID/GID 10001 for real secret mounts. Base updates require deliberate digest refresh, signature verification and re-scanning.

## Multiple servers and MEMORY ALPHA

Use TRUENAS_TARGETS_FILE with deployment/targets.example.json to configure independent targets in one process. Add a second entry with its own name, URL, key file and optional CA file; no code change is required. Metrics include target="configured-name" and readiness requires all configured targets to be fresh. Legacy single-target environment settings remain supported.

The new endpoint wss://memory-alpha.home.lab/api/current passes verified Rust WSS transport on the host and distroless container with an explicit, read-only Caddy CA file. The operator provisioned the dedicated prometheus_exporter account/key with Readonly Admin. Authorized native and UID-10001 container authentication and pool.query checks passed, with sanitized captured envelopes replayed offline. Separately authorized `check-system` runs also passed from the Mac and UID-10001 container; captured system.info replies report version 25.04.2.6. Authorized realtime preflight also passed from both environments, including owned unsubscribe; four sanitized realtime frames now replay offline; numerical comparison and integrated live operation remain pending. See docs/memory-alpha-integration.md for scope, evidence and operator steps. check-transport is a manual transport diagnostic, not an authentication test.

## Optional system collection

Set `TRUENAS_SYSTEM_ENABLED=true` to add scheduled system identity/version, uptime, physical total memory and 1/5/15-minute load metrics. Each connection must pass the 25.04.2.x compatibility gate before admitting requests. Pools and system retain independent snapshots/expiry; both are required for readiness in this mode. Unsupported or unvalidated versions suppress the affected target's measurements immediately. The default false preserves pool-only collection. See [configuration](docs/configuration.md), [metric catalog](docs/metric-catalog.md), and [system contract](docs/system-contract.md) for optional-value and failure policies. Realtime is not enabled by this switch. No live daemon or production deployment was performed for M3-C.

## Optional realtime collection

Set `TRUENAS_SYSTEM_ENABLED=true` and `TRUENAS_REALTIME_ENABLED=true` to add CPU, available-memory, network and ARC metrics from a separate version-checked `reporting.realtime` subscription. Interval defaults to two seconds; realtime TTL defaults to three intervals. Each event group has independent validation/freshness and becomes required for readiness. Missing values stay absent; down-link rates/speeds are omitted. Both flags default false, and mock mode stays network-free.

`check-realtime` is the fixed-output one-shot subscription preflight for a separately authorized endpoint. It validates fresh groups and unsubscribes only its owned ID. See [realtime contract](docs/realtime-contract.md) for exact methods, source mappings, bounded memory/cleanup, and the outstanding live/capture/resource acceptance gates. No realtime call or production change was made during implementation.

Authorized native and UID-10001 local Docker `check-realtime` runs now pass against MEMORY ALPHA, including fresh selected groups and owned unsubscribe acknowledgement. The command saves no payload. A subsequent temporary capture helper saved four sanitized notifications with provenance and successful offline replay. UI/Graphite comparison and full integrated runtime/resource acceptance remain pending. See [integration evidence](docs/memory-alpha-integration.md).

An explicitly authorized one-shot public chart diagnostic is available as the Cargo example public_chart_comparison. It requires a new output-file path. Read docs/public-chart-comparison-results.md and docs/reporting-api-comparison-plan.md before use.

The complete Grafana dashboard is in dashboards/truenas-exporter.json. The authorized local Docker Desktop setup runs Prometheus, Grafana 13.2.2 and the live exporter together in metric-simulator-observability. See docs/operations.md for paths and lifecycle instructions.

The operator-authorized Linux observability stack now runs exporter image 0.8.0 on 10.10.10.60 and scrapes it internally as job truenas-api-exporter. See docs/operations.md for the installed configuration and rollback record.
