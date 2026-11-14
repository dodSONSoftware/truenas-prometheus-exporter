# TrueNAS SCALE Rust Prometheus Exporter — Codex Development Handoff

**Current navigation (November 14, 2026):** This is the original brief and historical design evidence. Preserve its operating boundaries; use [current status](docs/project-status.md), [implementation plan](docs/implementation-plan.md), [conversation handoff](docs/handoffs/2026-11-14-handoff.md) and [restart prompt](docs/handoffs/2026-11-14-restart-prompt.md) for implemented behavior, clarified defaults/contracts and the next task. Proposed names/defaults below are not claims of current implementation.

**Status:** Implementation specification / initial development brief  
**Target release:** v0.1.0  
**Revised:** 2026-10-27 (Codex monorepo and API contract review)  
**Primary target:** TrueNAS SCALE **25.04.2.6** (MEMORY ALPHA)  
**Implementation:** Rust, standalone container deployed with Docker Compose  
**Operating model:** Strictly read-only, remote, API-based monitoring  
**Repository:** One Git monorepo rooted at `truenas_metrics_project/`; one primary Codex source folder  
**Absolute development boundary:** Codex must not connect to, query, probe, test against, or modify MEMORY ALPHA (`10.10.10.7`) without prior explicit operator authorization. No changes to live Prometheus, Grafana, or Compose without approval.

## 1. Mandate

Build a production-minded **general-purpose TrueNAS SCALE Prometheus exporter written in Rust**. It connects remotely to TrueNAS's supported JSON-RPC 2.0 WebSocket API, collects current state and performance telemetry, and serves typed, documented Prometheus metrics. It runs on the existing observability Ubuntu VM as a Docker Compose service. **Do not install an application, agent, or package on TrueNAS.**

Use [Unknowlars/truenas-scale-api-prometheus-exporter](https://github.com/Unknowlars/truenas-scale-api-prometheus-exporter) for ideas on coverage, API methods/events, deployment, and validation. **Do not port its Python monolith, copy its metric schema without review, or assume compatibility with TrueNAS 25.04**; the reference project documents testing against 25.10.2.

The first release should be useful by itself and extensible, without building an elaborate plugin framework. **No TrueNAS administrative writes, no application storage, no database, no browser UI.** The service *reports metrics*, including storage **metrics** (capacity, health, scrub progress), but never stores user data or manages ZFS pools. This project is separate from the immediate Grafana dashboard construction, which can continue using current metrics while this exporter is developed.

### Definition of success

- An operator can configure a read-only TrueNAS API identity, build the Rust container, start it via Compose, and scrape `/metrics` from Prometheus.
- Offline tests validate all collectors against sanitized TrueNAS 25.04.2.6 fixtures. Any later validation against MEMORY ALPHA, its UI, `midclt`, or Graphite telemetry requires separate explicit operator authorization; it is not part of autonomous Codex execution.
- Failures, stale data, permission gaps, unsupported APIs, and bad units are observable; **never disguise missing telemetry as zero**.
- The legacy Graphite path continues running unchanged until an explicit later migration decision.

## 2. Known environment and discovery evidence

| Item | Known value |
|---|---|
| NAS hostname/identity | MEMORY ALPHA / `truenas` |
| TrueNAS address | `10.10.10.7` |
| TrueNAS version | **25.04.2.6** (verified with `/etc/version`) |
| Observability VM | `10.10.10.60`, Ubuntu, Docker Compose |
| Compose directory | `/mnt/observability-stack` |
| Existing services | Grafana, Prometheus, Loki, `graphite-exporter`, other monitoring components |
| Existing Graphite image | `prom/graphite-exporter:v0.16.0` |
| Graphite ports | **9108–9109** already occupied; **do not reuse** |
| Proposed new exporter port | **9110** inside the Docker network |
| Existing Prometheus data source in Grafana | Name `prometheus`, UID `afpqtre5ghmgwe` |
| Existing Graphite scrape job | `truenas-graphite-10.60`, instance `10.10.10.60:9108` |
| NAS primary interface | `enp42s0` |
| CPU samples | 12 per-core series, one aggregate series; see below |
| NAS main ZFS pool | `tank`; RAIDZ2 with six disks |
| Grafana MCP | Functional; handled separately from this exporter |

### Current Graphite-derived telemetry (baseline for comparison)

- Host CPU: `truenas_truenas_truenas_cpu_usage_cpu_cpu` and `...cpu_cpu0` through `...cpu_cpu11`; example aggregate `0.1`, per-core mean approximately `0.325` in apparent percent units. Do not assume the new reporting event's CPU shape matches Graphite.
- Load averages: `truenas_truenas_system_load_load1`, `load5`, `load15`; approximately 11 despite near-idle CPU samples. **Investigate disparity; do not label load average as CPU utilization.**
- Temperatures: `truenas_truenas_cputemp_temperatures_cpu*`, all reported `30` in a sample, not independently verified.
- Physical RAM: total `63,093,960,000` bytes, available `15,732,212,800` bytes (~75.07% usage by `1 - available/total`). Account for ARC rather than assuming this indicates memory pressure.
- ZFS ARC size: `42,298,870,000` bytes; ARC available `9,519,400,050`, ARC free `11,732,222,950` (different semantics; do not combine blindly).
- Graphite network readings: receive `10.699167`, sent `-19.3859995`, interface speed `1e6`, inherited ambiguous Graphite units. Prefer explicit units from 25.04's `reporting.realtime` API event for the new exporter.
- Prometheus `up{job="truenas-graphite-10.60"}` has been `1`, which signifies **exporter scrape success**, not full NAS health.

### Verified TrueNAS `pool.query` sample

Direct `sudo midclt call pool.query` returned for `tank`:

| Field | Value |
|---|---|
| `name` | `tank` |
| `status` | `ONLINE` |
| `healthy` | `true` |
| `warning` | `true` |
| `status_code` | `FEAT_DISABLED` |
| `size` | `24000277250048` bytes |
| `allocated` | `12549572898816` bytes |
| `free` | `11450704351232` bytes |
| `fragmentation` | `13` |
| `scan.function` | `SCRUB` |
| `scan.state` | `FINISHED` |
| `scan.errors` | `0` |
| Topology | RAIDZ2, 6 data disks, reported ONLINE |

Pool allocation is approximately **52.29%**. Crucially, `healthy=true` **and** `warning=true` are both meaningful: `FEAT_DISABLED` indicates pool features not enabled, not a degraded disk. Export these independently. The shell's `zpool list` also showed `boot-pool` but `pool.query` sample showed only `tank`; **do not presume the API will return boot-pool**. Model absence explicitly.

## 3. Mandatory platform/API compatibility

- Implement specifically against **TrueNAS 25.04.2.x** first, using documentation and fixtures from that version.
- API transport is **JSON-RPC 2.0 over WebSocket** at an endpoint such as `wss://10.10.10.7/api/current` (endpoint must be configurable).
- Authenticate with a dedicated API key tied to a least-privilege account. The **25.04.2 `auth.login_ex`** schema specifies `mechanism: "API_KEY_PLAIN"`, **`username`**, and **`api_key`**. Implement this exact typed shape and inspect the `response_type` (`SUCCESS`, `AUTH_ERR`, `EXPIRED`, `OTP_REQUIRED`, `REDIRECT`) rather than treating every JSON-RPC result as successful login. Verify actual minimum roles/privileges on the 25.04.2.6 appliance before deployment; do not copy later-version role assumptions.
- Use TLS verification by default. Support a trusted private CA; do not silently fall back to insecure `ws://` or disable certificate checking. IP-certificate name mismatch must be diagnosed clearly.
- Implement asynchronous request IDs, request/response correlation, subscription/event demultiplexing, bounded timeouts, ping/pong if needed, connection-loss detection, exponential backoff with jitter, and graceful cancellation.
- **Strict read-only allowlist** of API methods/events; reject all non-allowlisted methods in code. The credential must independently enforce least privilege on the TrueNAS side.
- API methods and event payloads should be version-gated; unsupported collection groups fail independently and provide operational metrics.
- Inspect the version-specific `/api/docs/` provided by the NAS or `https://api.truenas.com/v25.04/` during development to confirm schemas.

### Crucial realtime detail

Official TrueNAS 25.04 `reporting.realtime` event provides CPU, network, virtual memory and ZFS ARC statistics. The interface portion documents **`speed` in megabits/s** and **`received_bytes_rate` / `sent_bytes_rate` in bytes/s**. Subscribe with a configurable interval of at least **2 seconds**. Avoid applying `rate()` to instantaneous byte-rate fields; do not carry Graphite's negative TX convention into the canonical exporter uncritically. Check actual event values against the documentation and observed interface activity.

## 4. Scope and phased collectors

Build a **general-purpose exporter with independent collectors**, but prioritize a coherent v0.1 release:

| Group | Priority | Source strategy | Scope |
|---|---|---|---|
| Exporter self-monitoring | P0 | Internal | connection/auth status, collector success and freshness, duration, error counters |
| System | P0 | Documented read-only API calls | hostname, software version, uptime, overall API reachability |
| CPU | P0 | `reporting.realtime` plus system info | aggregate/per-core utilization, 1/5/15-minute load if verified |
| Memory / ARC | P0 | `reporting.realtime` and documented reporting fields | total, available, used when semantics verified, ARC size |
| Network | P0 | `reporting.realtime` | interface state, negotiated Mbps, RX/TX bytes/s; exclude/label virtual interfaces rather than arbitrarily summing |
| ZFS pools | P0 | `pool.query` periodic | size, allocated, free, healthy, warning, status, fragmentation, scrub status/errors |
| Disks | P1 | `disk.query` and documented read-only health sources | inventory, device temperature and health, only where reliably accessible |
| Services and alerts | P1 | Specific read-only API calls | enabled/running services, alert severity/count |
| Applications, tasks, VMs | P2 | Version-supported methods/events | only after P0/P1 are stable |
| Arbitrary discovery/generic recursive metrics | **Out of scope** | — | no free-form `METHOD_*`, `EVENT_*`, API field flattening |

Implement P0 end-to-end before expanding P1. It is acceptable to mark an optional P0 metric unsupported when the 25.04 API lacks it; do not invent values or block all collectors.

### Scheduling defaults

- Real-time subscription: API-defined interval (start at **5 seconds**, configurable, minimum 2).
- Pools and system state: **60 seconds**.
- Disk/service/alert state when implemented: **60–300 seconds**, per cost and source semantics.
- Prometheus scrape: **30 seconds**.
- Configurable API deadlines and readiness freshness thresholds. Set finite collector-specific cache TTLs (for example, start with **3 × polling/subscription interval**, then tune based on observed jitter); validate they exceed normal scheduling jitter and document exact defaults.
- **No TrueNAS API request directly inside the Prometheus HTTP scrape handler.** Publish immutable/atomic cached snapshots.

## 5. Proposed Prometheus metric contract

Prefer standard Prometheus names, types, and units. These are **design names to implement**, not existing metrics. Document each with `HELP`, `TYPE`, source method/event, semantic definition and sample labels.

| Name | Type | Meaning |
|---|---|---|
| `truenas_exporter_collection_success{collector}` | gauge 0/1 | most recent attempt outcome |
| `truenas_exporter_last_success_timestamp_seconds{collector}` | gauge | last successful validation/update epoch |
| `truenas_exporter_collection_duration_seconds{collector}` | gauge | duration of last attempt |
| `truenas_exporter_last_attempt_timestamp_seconds{collector}` | gauge | most recent attempt epoch |
| `truenas_exporter_snapshot_age_seconds{collector}` | gauge | elapsed age of last successful snapshot; omit until first success |
| `truenas_exporter_collection_errors_total{collector,reason}` | counter | bounded error categories |
| `truenas_api_connected` | gauge 0/1 | API transport connected/authenticated |
| `truenas_system_info{host,version}` | gauge 1 | stable system identity; avoid volatile labels |
| `truenas_system_uptime_seconds` | gauge | host uptime, not exporter uptime |
| `truenas_cpu_utilization_ratio{cpu}` | gauge | 0–1; distinguish aggregate `cpu` vs `cpu0` etc |
| `truenas_memory_total_bytes` | gauge | total host physical RAM |
| `truenas_memory_available_bytes` | gauge | available host memory with documented accounting |
| `truenas_zfs_arc_size_bytes` | gauge | current ARC size |
| `truenas_network_receive_bytes_per_second{interface}` | gauge | current RX rate |
| `truenas_network_transmit_bytes_per_second{interface}` | gauge | current TX rate, positive |
| `truenas_network_link_speed_bits_per_second{interface}` | gauge | Mbps converted to bits/s |
| `truenas_network_link_up{interface}` | gauge 0/1 | physical/link state; unknown must not become 0 |
| `truenas_pool_size_bytes{pool}` | gauge | total pool size |
| `truenas_pool_allocated_bytes{pool}` | gauge | allocated bytes |
| `truenas_pool_free_bytes{pool}` | gauge | free bytes |
| `truenas_pool_healthy{pool}` | gauge 0/1 | TrueNAS `healthy`, distinct from warnings |
| `truenas_pool_warning{pool}` | gauge 0/1 | TrueNAS `warning` |
| `truenas_pool_info{pool,status,status_code}` | gauge 1 | bounded condition metadata; avoid free-text detail |
| `truenas_pool_fragmentation_percent{pool}` | gauge | fragmentation 0–100 |
| `truenas_pool_scrub_errors{pool}` | gauge | errors from latest reported scan, **not** a lifetime counter |

Notes:

1. Use target labels provided by Prometheus (`job`, `instance`) and a stable `host` label only if explicitly configured and consistently applied; do not duplicate high-cardinality data needlessly.
2. No generated timestamps, error messages, user names, disk paths, GUIDs, or arbitrary JSON paths in label values without a reviewed bounded-cardinality contract.
3. Metrics are absent when unsupported/invalid. **Required freshness policy:** keep the last validated snapshot only for a configurable bounded TTL; after expiry, omit its TrueNAS measurement series while continuing exporter operational series. Always set the failing collector success to `0` and retain its last-success timestamp. Expose snapshot age and last attempt timestamp explicitly. A disconnected exporter must never indefinitely advertise old NAS measurements as current.
4. Do not emit `NaN`, `Inf`, negative utilization, or nonsensical counters. Validate ranges, avoid silently clamping schema errors.
5. Preserve separate healthy and warning pool signals. Avoid labeling `FEAT_DISABLED` as device degradation.
6. Do not create Prometheus counters from API measurements that are already instantaneous rates.

### Example PromQL for future dashboard migration

```promql
100 * truenas_cpu_utilization_ratio{cpu="cpu"}
100 * (1 - truenas_memory_available_bytes / truenas_memory_total_bytes)
truenas_zfs_arc_size_bytes
truenas_network_receive_bytes_per_second{interface="enp42s0"}
truenas_network_transmit_bytes_per_second{interface="enp42s0"}
100 * truenas_pool_allocated_bytes{pool="tank"} / truenas_pool_size_bytes{pool="tank"}
truenas_pool_healthy{pool="tank"}
truenas_pool_warning{pool="tank"}
time() - truenas_exporter_last_success_timestamp_seconds{collector="pools"}
```

Queries are **illustrative** until labels and names are implemented and validated. Grafana panel thresholds are a separate concern, especially for ARC-aware memory reporting.

## 6. Rust design and dependencies

**One Cargo package, one executable.** Use a restrained modular design (composition, fail-fast configuration validation, typed interfaces). Preferred crates, subject to current API and maintenance review:

- `tokio` — async runtime, tasks, cancellation and deadlines.
- `tokio-tungstenite` + `rustls` / appropriate TLS integration — WebSocket JSON-RPC 2.0.
- `serde` / `serde_json` — typed, version-aware response decoding; avoid opaque generic flattening.
- `axum` — HTTP endpoints.
- `prometheus-client` — Prometheus/OpenMetrics metrics encoding.
- `tracing` / `tracing-subscriber` — structured safe logging.
- `thiserror` / `anyhow` — categorized library/application errors.
- `clap` or a small typed env configuration module — validation, no sprawling config framework.

Suggested **single-repository** structure (the project is one Git repository, not three independent ones):

```text
truenas_metrics_project/              # Git root / sole primary Codex source folder
├── .git/                              # Created by git init, never hand-authored
├── application/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs
│   │   ├── api/{mod.rs,client.rs,protocol.rs,auth.rs,subscriptions.rs}
│   │   ├── collectors/{mod.rs,system.rs,realtime.rs,pools.rs}
│   │   ├── metrics/{mod.rs,registry.rs,encoding.rs}
│   │   ├── scheduler.rs
│   │   └── http.rs
│   └── tests/{fixtures/,api_client.rs,collectors.rs,metrics.rs,failures.rs}
├── dashboards/
│   ├── json/                          # Version-controlled Grafana dashboard JSON
│   └── provisioning/                  # Optional Grafana provisioning examples
├── deployment/
│   ├── Dockerfile
│   ├── compose.yaml                   # Self-contained local/development deployment
│   ├── compose.observability.example.yaml
│   ├── prometheus-scrape.example.yaml
│   └── .env.example
├── docs/
│   ├── architecture.md
│   ├── metric-catalog.md
│   ├── compatibility.md
│   ├── operations.md
│   ├── configuration.md
│   └── implementation-plan.md
├── scripts/                           # Only scripts with concrete, tested value
├── .github/workflows/ci.yaml          # If repository hosted on GitHub
├── AGENTS.md                          # Concise root instructions for Codex
├── .dockerignore
├── .editorconfig
├── .gitignore
├── LICENSE                            # Select license explicitly; do not guess
├── README.md
└── CHANGELOG.md
```

Do not add empty modules, stub CI or speculative tooling merely to match this illustration. Keep `application/` as **one Cargo package** and set the Docker build context to the Git root so `deployment/Dockerfile` can copy `application/Cargo.toml`, `application/Cargo.lock`, and `application/src/`. Run `cargo` from `application/`, or use `--manifest-path application/Cargo.toml` from the root.

All Rust files should have meaningful functionality, not scaffolding-only placeholders. Keep inline comments concise and explain non-obvious decisions. No speculative interfaces or plugin loading system before needed.

### API client contract

- Centralized transport with a finite in-flight request map, correlation IDs, and bounded response sizes.
- Typed method wrappers for the **explicitly allowlisted** read-only operations.
- Stream handling decoupled from requests. Slow consumers must not block the WebSocket read loop; use bounded queues and drop/recover semantics as appropriate.
- Reconnect with jitter; retry only safe/idempotent reads; do not create tight retry loops.
- Reauthenticate and resubscribe after connection loss. Track subscription freshness.
- Version/protocol mismatches are surfaced and logged once with clear diagnostics, not hidden by defaults.

### Collector contract

- Each collector independently validates, timestamps, and publishes a snapshot.
- Atomic updates prevent mixed partial data. Missing fields never overwrite real values with fabricated zero.
- Use the API payload's documented units, convert exactly once in collection code, and exercise conversions in unit tests.
- The `/metrics` route serves current cached state; no blocking TrueNAS calls.
- Granular readiness: `/healthz` is process liveness; `/readyz` reports whether required collector data is sufficiently fresh; `/metrics` remains available to report exporter failures if TrueNAS is down. Do not wire a runtime restart loop to `/readyz`: NAS outages should show as degraded monitoring, not endless exporter restarts.

## 7. Configuration and security

Recommended operator settings (final names may be refined, but document all defaults):

```dotenv
TRUENAS_WS_URL=wss://10.10.10.7/api/current
TRUENAS_HOST=memory-alpha
TRUENAS_USERNAME=exporter-monitor
TRUENAS_API_KEY_FILE=/run/secrets/truenas-api-key
# TRUENAS_CA_CERT_FILE=/run/secrets/truenas-ca.pem
EXPORTER_LISTEN=0.0.0.0:9110
TRUENAS_REALTIME_INTERVAL_SECONDS=5
TRUENAS_POLL_INTERVAL_SECONDS=60
TRUENAS_API_TIMEOUT_SECONDS=10
RUST_LOG=info
```

**Verified:** TrueNAS 25.04.2 `API_KEY_PLAIN` requires a `username` and an `api_key`. `TRUENAS_USERNAME` is mandatory in normal mode. Do not confuse API authentication success with authorization for individual methods/events. Confirm least-privilege roles with the actual appliance.

- No secrets committed; use file mounts with correct UID/GID ownership and restrictive permissions (prior Grafana MCP integration encountered `root:root` `0600` secrets unreadable by UID 1000).
- Default deny for unsupported or mutating methods; a read-only service account is required even if the Rust API client itself restricts calls.
- TLS certificate verification must be on by default; use a CA mount / DNS name matching certificate. No insecure fallback.
- Runtime non-root, `read_only: true`, `no-new-privileges`, drop capabilities, optional tmpfs if needed, image pinned/versioned; avoid mounting Docker socket.
- Do not publish the metrics port on the LAN by default; use Docker-network-only `expose` and Prometheus service DNS.
- Never log API keys, auth frames, full unredacted API payloads, or user-identifying field values.
- Avoid installing anything on TrueNAS and do not modify TrueNAS configuration except operator-approved setup of a dedicated read-only API credential through its supported WebUI/API.

## 8. Docker and Prometheus integration

**Deployment target:** the Ubuntu observability VM `10.10.10.60`, with the **existing** Compose project `/mnt/observability-stack` as the intended eventual integration point. Development and CI must be independent of that project. Produce (a) a standalone Compose file for local/offline validation and (b) a reviewed opt-in integration fragment for the current stack. Do not create a competing production Compose stack by default, and never clobber the existing `compose.yaml`, `.env`, monitoring configuration, or secrets. The repository directory on the developer Mac is not assumed to exist on the observability VM.

Example integration fragment **for integration into the existing observability Compose project only after operator approval** (illustrative; verify build context relative to the Compose file, image user, ownership, health checker and network name when implementing):

```yaml
services:
  truenas-exporter:
    build:
      context: ./truenas_metrics_project
      dockerfile: deployment/Dockerfile
    image: local/truenas-rust-exporter:0.1.0
    container_name: truenas-exporter
    restart: unless-stopped
    init: true
    environment:
      TRUENAS_WS_URL: "wss://10.10.10.7/api/current"
      TRUENAS_HOST: "memory-alpha"
      TRUENAS_USERNAME: "exporter-monitor"
      TRUENAS_API_KEY_FILE: /run/secrets/truenas-api-key
      EXPORTER_LISTEN: 0.0.0.0:9110
      RUST_LOG: info
    volumes:
      - ./secrets/truenas-api-key:/run/secrets/truenas-api-key:ro
    expose:
      - "9110"
    read_only: true
    security_opt:
      - no-new-privileges:true
    networks:
      - observability
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"
```

Prometheus example:

```yaml
scrape_configs:
  - job_name: truenas-api-exporter
    scrape_interval: 30s
    static_configs:
      - targets: ["truenas-exporter:9110"]
```

In the actual observability stack, add the scrape job to the **existing** `scrape_configs` instead of replacing the file, **only after explicit approval**; confirm the Compose network exists and Docker DNS works. The sample `./truenas_metrics_project` build context is valid only if that directory was intentionally deployed adjacent to `/mnt/observability-stack/compose.yaml`; alternatively use a pinned built image or an approved resolved build path. Verify configuration with `docker compose config` before any rollout. Compose healthcheck implementation should use an available binary or exporter-supported self-check; do not assume a minimal distroless container has `curl`/`wget` or `/bin/sh`.

### Container build

- Reproducible multi-stage build with pinned dependency lockfile and appropriate MSRV.
- Small non-root runtime image with CA roots and no compiler/toolchain.
- Support linux/amd64; avoid architecture-specific unsafe assumptions.
- Include a `.dockerignore` excluding secrets, `target/`, VCS metadata and local env files.

## 9. Reliability, observability, and tests

Minimum requirements:

1. Valid/invalid auth, expired key, insufficient permissions, TLS trust/name mismatch.
2. Concurrent JSON-RPC requests with out-of-order responses; unknown IDs; error frames; malformed JSON; oversized frames.
3. Subscription establishment, event parsing, interruption, reconnection, resubscribe, and stale events.
4. Missing/null pool size, no pools, `healthy=true` and `warning=true` simultaneously, scan absent, `FEAT_DISABLED`, zero-sized pool.
5. CPU utilization units and aggregate/core differentiation. Report per-core and aggregate independently when supported; never use system load as CPU percent.
6. Memory/ARC accounting and unit conversions; label what represents physical memory vs cache.
7. Network rates and speeds per official reporting event, valid/invalid interfaces; preserve positive RX/TX rate semantics.
8. Collector partial failure isolation and cached data policy. Failures update `collection_success=0`; no fabricated zero values.
9. Deterministic `/metrics` exposition tests (HELP, TYPE, label sets, numeric units, bounded cardinality) and liveness/readiness behavior.
10. Graceful stop, restart, correct Docker permissions, no secret leakage to logs.
11. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and container build as CI quality gates.
12. Dependency/security audit where available (`cargo audit` or equivalent); track known risk and require approval to ship a risky dependency.

Use sanitized fixtures captured from **25.04.2.6** and fake WebSocket/JSON-RPC test servers; tests must not require a live NAS or API credentials. Add an opt-in live integration smoke test with no writes.

## 10. Development milestones and acceptance criteria

### M1 — Monorepo scaffold and local deployability

- Root Git repository, one primary Codex folder, concise `AGENTS.md`, `application/`, `dashboards/`, `deployment/`, `docs/`, shared CI/config/documentation.
- Rust binary with typed config and fail-fast startup.
- `/metrics`, `/healthz`, `/readyz` (with health semantics documented).
- Multi-stage Dockerfile, minimal standalone Compose, main-stack integration fragment, `.gitignore`, `.env.example`, README.
- CLI commands for Debian-based build, test, run, and Compose launch.
- Unit tests and lint/build pipeline.

**Acceptance:** Starts without TrueNAS credentials only in an explicit test/mock mode; normal deployment fails clearly without required credentials **but does not crash merely because a correctly configured NAS is temporarily unreachable**. Prometheus-format exporter self-metrics served successfully.

### M2 — Authenticated 25.04 API client

- WSS/TLS, JSON-RPC client, 25.04-compatible API-key login, error types, reconnect, bounded requests.
- Read-only allowlisted API calls and an initial `pool.query` client.
- Offline fixture tests and mock-server smoke tests only. A live read-only test is a separately authorized future activity, not part of this milestone by default.

**Acceptance:** Mock authenticated `pool.query` correctly parses the sanitized `tank` fixture and exercises failure behavior. Live comparison with MEMORY ALPHA is deferred until the operator explicitly authorizes that particular activity.

### M3 — Core collectors

- `reporting.realtime` CPU/memory/network/ARC event handling and pool/system polling.
- Canonical metrics and metadata documentation.
- Freshness/collector isolation.

**Acceptance:** P0 metrics are verified against documented schemas and sanitized fixtures with known units. Comparing to live TrueNAS UI or Graphite exporter is deferred pending explicit authorization. Document non-equivalent definitions.

### M4 — Operational hardening

- Credential/CA mount, graceful recovery, healthcheck, read-only container, resource budgets, log redaction, CI.
- Cardinality and failure testing; network isolation and documentation.

**Acceptance:** Container survives simulated TrueNAS outages and returns cleanly without manual intervention; Prometheus detects stale data.

### M5 — Expanded monitoring and Grafana handoff

- Add P1 collectors only after the 25.04 calls are established (disks, services, alerts).
- Provide example recording rules, starter Grafana dashboard JSON or dashboard integration instructions, and a migration mapping from old Graphite series.
- Maintain both exporter jobs in Prometheus throughout comparison; do **not** disable Graphite or rewrite existing dashboards automatically.

**Acceptance:** Demonstrated monitoring coverage and a documented go/no-go decision about replacing Graphite; no implicit deployment into production without user approval.

## 11. Codex working instructions

1. Start by inspecting the reference repository and the official **25.04** API documentation. Identify methods/event schemas actually supported on 25.04.2.6; write any version assumptions into `docs/compatibility.md`.
2. Produce an implementation plan and milestone checklist in `docs/implementation-plan.md`, then implement **M1 and M2 first**. Keep the code testable without the NAS. Proceed to M3 once API behavior is confirmed.
3. Prefer simple modules with explicit responsibilities over abstractions added for hypothetical backends. Apply DRY, KISS, YAGNI, SOLID and composition over inheritance.
4. **MEMORY ALPHA is off-limits.** Do not connect to `10.10.10.7` by any protocol (including WebSocket, HTTP, SSH, ping, or port probing); do not use its API, UI, or `midclt`; and do not run live smoke/integration tests or make changes. Only sanitized fixtures and local mock servers are authorized for development. Request separate explicit operator authorization before *any* interaction. Also do not reconfigure existing Compose services, change live Grafana dashboards, or disable Graphite without approval.
5. For every new metric, document the TrueNAS source field, units, type, expected labels, handling of missing values, and test.
6. Keep `README.md` professional: description; a **single Debian-based CLI block** showing install/build/test/run end to end; file inventory with one-line descriptions and source listings where practical; prioritized enhancements. Include `.gitignore` entries covering Rust, Node/TypeScript and Python (`target/`, `node_modules/`, `dist/`, `.venv/`, `__pycache__/`, `*.pyc`, `.pytest_cache/`, `.env`).
7. Maintain `CHANGELOG.md`, meaningful test fixtures, and complete example configuration. Do not hardcode the host/IP or credentials into the Rust code.
8. At each milestone report implemented files, test results, API calls used, compatibility findings, unsolved questions, and the next step. Never report unrun tests as passing.
9. If source API behavior or units conflict with this document, use **verified 25.04 documentation and sanitized fixtures**, explain the discrepancy, and update the spec rather than fabricating a value. Live read-only evidence may only be gathered following separate explicit operator authorization.
10. Present any user-required TrueNAS credential/setup tasks as precise operator steps. Do not request the secret itself in chat or persist it in Git.
11. Treat `truenas_metrics_project/` as the **single Git root and sole primary Codex source folder**. Do not create nested `.git/` directories in `application/`, `dashboards/`, or `deployment/`. Keep root `AGENTS.md` concise and apply shared instructions across the tree.
12. Make a small, reviewable initial commit after scaffold validation, only with operator-approved Git actions. Do not push, create a remote, publish releases, or enable deployments without approval. Use existing global Git identity; do not override it in the repo.
13. Keep dashboards in `dashboards/` but do not rewrite existing Grafana dashboards; any new dashboard JSON is an artifact for review, not an automatic live import. Keep container deployment files in `deployment/` and high-level documentation at the root/`docs/`.
14. Define explicit contracts for build context, relative paths, local secret mounts, environment variables, networking, and scrape target differences between local Compose and production integration. Never assume a local developer path exists on the target VM.

## 12. Research links and provenance

- [Reference project: Unknowlars/truenas-scale-api-prometheus-exporter](https://github.com/Unknowlars/truenas-scale-api-prometheus-exporter) — feature set, Docker setup, hybrid polling/event approach; maintained primarily against TrueNAS 25.10.2.
- [Official TrueNAS API hub](https://www.truenas.com/docs/scale/api/) — versioned API guidance.
- [TrueNAS 25.04.2 `auth.login_ex`](https://api.truenas.com/v25.04.2/api_methods_auth.login_ex.html) — verified `API_KEY_PLAIN` requires `username` and `api_key`; inspect `response_type`.
- [TrueNAS 25.04 `reporting.realtime`](https://api.truenas.com/v25.04/api_events_reporting.realtime.html) — CPU, memory, ARC, network; interface speed Mbps and RX/TX bytes/s.
- [TrueNAS 25.04 `pool.query` event schema](https://api.truenas.com/v25.04/api_events_pool.query.html) — pool fields and status semantics. Distinguish event docs from callable method docs.
- [Official TrueNAS API client compatibility matrix](https://github.com/truenas/api_client/blob/master/README.md) — API differences between 25.04 and later releases.

**Source distinction:** The concrete environment and sample metric values above are based on commands/results collected in the conversation and the uploaded `pool.query` response. Architecture, metric names, sampling intervals and code organization are proposed requirements, to be validated during implementation.

## 13. Design decisions and implementation boundaries (2026-10-27 review)

The preceding environmental measurements and TrueNAS pool samples are **historical discovery evidence**, not guarantees about the machine at deployment time. Do not independently verify the live appliance version or values: any future live verification requires explicit operator authorization. Do not assume the NAS remains on 25.04.2.6 after an operator upgrade.

### Confirmed versus still-to-verify

- **Confirmed from versioned documentation:** `auth.login_ex` on TrueNAS 25.04.2 supports `API_KEY_PLAIN` with a required `username` and `api_key`; `reporting.realtime` supports a minimum 2-second interval and publishes network throughput in bytes/second and speed in megabits/second. See sources in §12.
- **Still to verify against the actual appliance:** TLS endpoint availability and certificate identity, service-account authorization for each collector, actual CPU/memory field semantics, pool query response variance, full event envelope, how the WebSocket server handles subscriptions and reconnects, and exact permissions needed for `pool.query` and reporting events.
- **Never infer:** an event's schema is automatically identical to its corresponding callable method; a working Graphite reading validates a new metric's units; a success response to authentication implies collection privileges.

### Outage policy and secure operations

- Fail fast on **static configuration errors** (missing username/key file, invalid intervals, malformed URL, unreadable CA/key file). On transient network/API failures after valid configuration, keep the process alive, emit failure/status metrics, reconnect with bounded backoff, and make `/readyz` fail when required snapshots expire.
- Avoid blocking the WebSocket read loop while collecting or encoding metrics. Scope tasks and channels with finite limits; protect against unbounded memory consumption and label explosion.
- Treat `/metrics`, `/healthz`, and `/readyz` as internal-network endpoints. Avoid sensitive diagnostic text in HTTP responses. Container liveness checks use `/healthz`; readiness is for monitoring/traffic decisions, **not** an automatic restart trigger.
- Fixture captures must be sanitized: no API keys, tokens, raw host-identifying metadata, user information, disk serials, or confidential paths in Git.
- A version bump or schema drift should result in explicit validation/test updates, not broad catch-all decoding that silently hides incompatibilities.

### Git and delivery policy

- `main` is the primary branch; use small topic branches when warranted. Conventional Commits are preferred but not a functional requirement.
- Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and a Docker build before claiming the milestone complete. If tools are unavailable, state that tests/build are **not run**.
- CI must not require secrets, the home LAN, or the live TrueNAS appliance. No live smoke tests are authorized by this handoff, including read-only tests. If the operator later authorizes a specific test, keep it separately gated and strictly read-only.
- Dashboards, recording rules, and alert rules remain artifacts for review until separately authorized for deployment. Preserve Grafana/Graphite continuity by default.

---

**Initial Codex instruction:** MEMORY ALPHA (`10.10.10.7`) is strictly off-limits: no network connections, API calls, probes, live tests, or modifications without separate explicit operator authorization. Use sanitized fixtures and local mocks only. Work within the **existing Codex project** whose one primary source folder is `truenas_metrics_project/`; initialize **one Git repository at that root** only if not already initialized. Read this handoff, write a concise root `AGENTS.md`, inspect the reference implementation and TrueNAS 25.04.2 API contracts, record the implementation plan, implement Milestone 1, then move to Milestone 2. Keep the existing Grafana dashboard construction and Graphite pipeline untouched; request approval for any production environment or credential change.
