# Local validation record

**Current evidence index:** [project-status.md](project-status.md) and [implementation-plan.md](implementation-plan.md). This is a dated ledger; later entries supersede earlier pending statements. Latest functional baseline 069450d passed 77 regular Rust tests and 11 Python tests, with four sustained scenarios ignored by default. Hosted CI, combined live-daemon/numerical/minimum-role acceptance and universal resource guarantees remain unestablished.

## Original review ledger — November 13, 2026 snapshot

Fourteen findings have implementation fixes. TN-02 now has authorized native/container pool captures and offline replay through the production client and HTTP metrics/readiness. Version 25.04.2.6 was initially operator-confirmed; the later M3-B system captures also report it through the API. Earlier results below retain their original scope.

| Finding | Commit | Disposition |
| --- | --- | --- |
| TN-01 | 4be109e | Implemented and locally verified |
| TN-02 | 4d7a29f | Captured full sanitized responses; offline replay passed |
| TN-03 | 0bfcab0 | Implemented and locally verified |
| TN-04 | 82588c3 | Implemented; finite full-process operational validation passed |
| TN-05 | 4750a91 | Implemented and locally verified |
| TN-06 | be2b94f | Implemented and locally verified |
| TN-07 | 8009702 | Implemented; finite full-process operational validation passed |
| TN-08 | 9a0b534 | Implemented and locally verified |
| TN-09 | b0ec1fe | Implemented and locally verified |
| TN-10 | b8ae644 | Implemented and locally verified |
| TN-11 | a036f4d | Implemented and locally verified |
| TN-12 | 9073311 | Implemented and locally verified |
| TN-13 | 9b22c73 | Implemented and locally verified |
| TN-14 | a782c14 | Implemented and locally verified |
| TN-15 | b2e9919 | Implemented and locally verified |

The operational continuation verifies sustained independent TLS connections, outage recovery, full-process shutdown under backpressure, Rust 1.88 and refreshed security gates. Hosted CI requires an authorized GitHub run. Authenticated pool compatibility passed. Broader methods/events and eventual deployment acceptance remain open.

Date: 2026-10-27 (America/Chicago). Host: macOS arm64. Toolchain: rustc 1.99.1 and cargo 1.99.1. The one Git root is truenas_metrics_project/; there are no nested repositories. Original handoff preserved.

## Passed

- `cargo fmt --check`
- `cargo clippy --locked --offline --all-targets -- -D warnings`
- `cargo test --locked --offline`: 16 tests passed, 0 failed (9 API tests, 2 configuration tests, 1 process test, 4 state/HTTP tests).
- `cargo build --locked --offline --release`
- Separate offline mock executable smoke: /readyz 200, synthetic /metrics, API connected=0, built-in healthcheck, clean SIGTERM exit. Normal mode without required settings failed startup as expected.
- `sh -n scripts/check.sh`
- Repository-root inspection confirms one .git directory. No commit or staging was performed.

Tests exercise synthetic authenticated pool responses, out-of-order concurrent requests, unknown IDs, bounded in-flight requests, permission and unsupported errors, invalid/expired authentication, malformed and oversized frames, stalled responses, cancellation during authentication/backoff, reconnect and reauthentication. Loopback TLS tests cover trusted private CA, untrusted certificate and certificate-name mismatch. Process tests verify HTTP remains live during permission failures, readiness stays false without valid data, healthcheck does not need credentials, trace-level logs exclude synthetic credentials/raw auth frames/remote errors, and SIGTERM shuts down cleanly.

State/HTTP tests cover exact TTL expiry with no new attempt, last-success retention, failure grace periods, empty pool lists, missing/null/invalid values, simultaneous healthy/warning, metadata normalization and label escaping. Initial tests caught missing label escaping; it was fixed and the regression passes.

## Item 1 follow-up validation

Docker Desktop 4.97.0 was installed on the Mac using the official app installer; the engine is Docker 29.9.2 and Compose 5.6.1. All container checks explicitly selected the local desktop-linux context. The default Rust toolchain was not changed.

Passed after the PEM dependency migration:

- `cargo +1.88.0 fmt --check`
- `cargo +1.88.0 clippy --locked --offline --all-targets -- -D warnings`
- `cargo +1.88.0 test --locked --offline`: all 16 tests pass.
- linux/amd64 release build inside the pinned Rust 1.88 container, running through Docker Desktop on Apple silicon.
- `docker --context desktop-linux compose -f deployment/compose.yaml config --quiet`
- `python3 scripts/check-container.py --docker /Applications/Docker.app/Contents/Resources/bin/docker --context desktop-linux`: HTTP liveness/readiness/metrics, Docker healthcheck, UID 10001, read-only filesystem, zero effective capabilities, no-new-privileges, installed MIT license and clean SIGTERM exit. The temporary container was removed.
- Strict cargo-audit: 0 vulnerabilities, 0 warnings. An initial unmaintained PEM wrapper warning was resolved.
- Superseded Debian scan: 1 critical, 56 high, 87 medium, 85 low, 1 unknown package findings. The runtime was then replaced with verified, digest-pinned distroless cc-debian13; current scan: **0 critical, 0 high**, 23 medium and 8 low. The explicit high/critical gate passes without exclusions; see security-validation.md.
- MIT package metadata and exact requested copyright verified.

## Not run or not established

- GitHub CI has not been executed; the workflow now includes strict RustSec auditing, the same shell-free container checks and a pinned Trivy high/critical gate.
- Native execution on the eventual Ubuntu host has not been performed. linux/amd64 container execution here uses the Mac's local Linux VM/emulation.
- Captured sanitized 25.04.2.6 payloads have not been supplied. `fixtures/truenas-25.04.2.6/` contains intake instructions only; test JSON remains synthetic.
- Medium/low runtime findings remain tracked. Security results are specific to the pinned image and scanned database; refresh and re-scan before release.
- Runtime version discovery and realtime subscription semantics remain M3 work.
- Long-running resource budgets, actual deployment secret ownership, appliance roles and production network/DNS remain later validation gates.

Initial offline validation did not contact MEMORY ALPHA. Subsequent explicitly authorized transport-only Caddy endpoint tests are recorded below. No production service, monitoring configuration, dashboard or credential was changed. No deployment, commit, push or publication was performed.

## Distroless follow-up

- Runtime base signature verified with Cosign 3.1.4 against the exact pinned digest and documented Google identity.
- Repeated shell-free runtime checks pass for mock mode and normal mode with mounted synthetic credentials/private CA and no network access. A mounted test-only Rust probe verifies actual UID/GID, capability sets, EROFS, no-new-privileges and absence of shell/package tools. It is not in the runtime image.
- Clean SIGTERM in both cases; no test container remains.
- Rust 1.88 formatting, strict Clippy, all 16 tests and strict RustSec audit pass after the container change. No application source or dependency changes were needed.
- Docker Compose config validates. Runtime image linux/amd64 build passed; image size changed from 129.3 MiB to 49.6 MiB (Docker-reported).

## Multi-target and Caddy integration follow-up

Historical multi-target suite: **22 tests passed**, 0 failed (10 API, 5 configuration, 1 multi-target process, 1 process/redaction, 5 state/HTTP tests). Both Rust 1.88 and installed stable strict Clippy checks pass; formatting includes the standalone runtime probe.

The new process test runs two local TLS mock servers with different CAs and credentials, identical pool names and different values. One target's later permission failures expire only its own measurements while the other continues updating. Readiness fails after the required target expires; liveness/metrics remain available. Trace logs exclude both credentials and account names. Descriptor/EOF uniqueness and config size/count/name/secret-field boundaries are tested.

The final linux/amd64 distroless build is `sha256:2dc783768b04a75364f5efc84070b74d516789c30daab01584591cae1c293ec6`, available as local/truenas-rust-exporter:multi-target-validation. Its standard runtime checks pass, including a third network-disabled case that loads two configured targets and exposes separate status metrics. Docker Compose config checks pass for the standalone file and the opt-in integration fragment with a temporary network declaration used only for syntax validation.

Live tests authorized by the operator: verified Rust WSS upgrade (HTTP 101) through memory-alpha.home.lab on the host and the actual final container, using the public Caddy root exported from trusted macOS certificates. The container rejects the endpoint without the private CA. No authentication or JSON-RPC method frames were sent. The dedicated account/key do not exist yet; live auth.login_ex, pool.query, actual role permissions and captured appliance fixtures remain unverified. See memory-alpha-integration.md.

Rebuilt image scan: 14 OS packages; 0 critical, 0 high, 23 medium and 8 low findings. Strict Rust dependency audit returns 0 vulnerabilities and 0 warnings. CI is updated but has not run on GitHub. No TrueNAS, Pi-hole, Caddy or production observability configuration was modified.

## TN-03 pool response allocation (2026-11-13)

A temporary Rust release harness called the session's envelope decoder followed by the pool collector. Each case ran in a separate network-disabled linux/amd64 container on Docker Desktop with 128 MiB memory, swap disabled and 0.5 CPU. Concurrent workers started at a barrier and shared the input string. These are single-run cgroup memory peaks for the decode workload including harness startup and input construction. Full exporter memory with separate sockets, snapshots and scrapes requires a separate measurement.

| Workers | Pool entries per response | Baseline at 9b22c73 (bytes) | TN-03 (bytes) |
| ---: | ---: | ---: | ---: |
| 1 | 1 valid | 12,779,520 | 12,222,464 |
| 1 | 256 valid | 12,218,368 | 10,694,656 |
| 1 | 75,000 | 118,325,248 | 10,768,384 |
| 16 | 75,000 | OOM kill, exit 137 confirmed by Docker | 13,402,112 |

The oversized case used a 975,037-byte JSON-RPC response with 75,000 single-field pool objects. Each corrected worker returned a schema error. The loopback regression also exercises 0, 1, 256, 257 and 75,000 entries, retaining the previous snapshot until its exact monotonic TTL and successfully collecting again on the same connection. All 25 Rust tests, formatting, strict Clippy and the linux/amd64 runtime image build passed.

## TN-04 concurrent scrapes (2026-11-13)

A temporary release harness populated 16 targets with 256 pools each and all seven pool metric families, served the production router on loopback, and issued simultaneous HTTP scrapes. A synthetic collection-state updater ran every 5 ms. Each run used a separate linux/amd64 Docker Desktop container capped at 128 MiB, no swap and 0.5 CPU. The client and server shared the container; memory includes response buffers on both sides. These single-run measurements are workload observations with host scheduling variation.

| Readers | Baseline successful scrapes | Corrected successful scrapes | Baseline peak bytes | Corrected peak bytes | Baseline health latency ms | Corrected health latency ms |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1 | 1 | 19,791,872 | 22,855,680 | 19 | 31 |
| 2 | 2 | 1 | 32,833,536 | 20,766,720 | 174 | 132 |
| 4 | 4 | 1 | 39,243,776 | 19,271,680 | 462 | 56 |
| 8 | 8 | 1 | 48,160,768 | 21,004,288 | 661 | 87 |

Successful responses were about 2,032,280 bytes. Excess corrected requests received 503. At eight readers the baseline took 683 ms with 6 updater iterations; the corrected run took 136 ms with 9 iterations. This experiment measures collection-state progress, not authenticated polling throughput. Admission caps active encoding, including abandoned-request encoders. It does not cap open sockets or all completed bodies retained by slow readers.

Formatting, strict Clippy and all 33 tests passed, including busy-encoder rejection, availability of health/readiness, recovery of encoder admission, and existing exact TTL and per-target expiry tests. The linux/amd64 runtime image build and container runtime checks passed.

With eight clients delaying reads for 500 ms, the corrected run admitted one scrape, returned 503 to seven, completed in 512 ms, peaked at 22,192,128 bytes, served health in 30 ms and recorded 66 updater iterations. This finite delayed-read case does not establish a bound for indefinitely stalled clients or test shutdown under socket backpressure; TN-07 retains that investigation.

## TN-07 HTTP drain deadline (2026-11-13)

A real loopback HTTP regression uses the production metrics router with 16 targets, 256 pools each, and 128-byte pool names. The client advertises a 1 KiB receive buffer, reads only the first 256 response bytes, and leaves the response open. With the original drain implementation the server failed to return within the three-second test budget. With a two-second post-cancellation drain deadline, the same case returned in 2.002 seconds.

A companion test reads a complete 4 MiB response while cancelling the server and verifies every body byte arrives. Formatting, strict Clippy and the full 35-test suite pass. Existing process-level tests independently exercise SIGTERM and worker/socket shutdown. The slow-reader regression cancels the production HTTP lifecycle function directly; it does not measure complete SIGTERM-to-process-exit time under maximum metrics load. Main joins cancelled workers after the drain and process runtime teardown releases remaining HTTP tasks. Already-running blocking encoding jobs may also contribute to total exit time.

## TN-08 transport entry points (2026-11-13)

Authenticated sessions and the credential-free transport diagnostic now use one socket-opening function. It reapplies the production URL policy before TCP connection, even when a Rust caller directly constructs the public configuration structs. Direct-entry tests reject plaintext, URL userinfo, query strings and fragments and verify the loopback listener receives no connection.

Protocol fixtures now use verified WSS with a shared synthetic loopback certificate and explicit test trust roots. Independent TLS cases still reject an untrusted certificate or wrong hostname, and the diagnostic test observes a close frame as its first frame. Formatting, strict Clippy and all 36 Rust tests passed.

## TN-09 selective environment loading (2026-11-13)

Unix child-process tests start normal and mock exporters with invalid Unicode in unrelated environment keys and values and successfully query local liveness before SIGTERM. Supported textual settings produce redacted setting-specific errors, including logging, listen address, username and transport URL. The healthcheck follows the same Unicode handling. The transport diagnostic ignores binary credential-path values, uses only transport settings, and reaches a deliberately unavailable loopback port without an environment panic.

File-based targets now pass parsed fields and the common timeout directly to validated builders. Existing target credential/CA isolation and file-boundary tests still pass. The refactor preserves transport validation before credential-file validation. Formatting, strict Clippy and all 37 tests passed.

## TN-10 healthcheck status line (2026-11-13)

The executable healthcheck now reads through CRLF across partial TCP reads with a fixed 128-byte status buffer. A subprocess regression splits the status prefix across writes, verifies an exactly 128-byte valid line, and rejects a 129-byte line, non-200 status, early EOF and bare LF. A peer holding the socket open triggers the existing three-second overall timeout. The test runs without API credentials.

Formatting, strict Clippy, all 38 tests, the linux/amd64 image build and the local container runtime suite passed.

## TN-11 authenticated retry stability (2026-11-13)

Each connection attempt starts without a stability timestamp. The client sets it only after open_authenticated succeeds, and the retry calculation resets after 120 seconds from that timestamp. A controlled-time unit test advances through repeated 300-second unsuccessful-attempt intervals, verifies exponential jitter ranges through the 60-second cap, retains backoff at 119 seconds of authenticated uptime and verifies reset at exactly 120 seconds. Existing loopback reconnect and cancellation-during-authentication/backoff tests pass.

Formatting, strict Clippy and all 39 Rust tests passed.

## TN-12 RPC envelope validation (2026-11-13)

Errors now require a string message as well as an integer code. Notifications accept omitted parameters, objects and arrays; scalar or null parameters fail the session as protocol errors. Decoder tests retain valid permission/unsupported/API categories and arbitrary valid error data.

Loopback TLS regressions verify malformed errors and notifications fail both pending callers with Protocol, close the affected session and recover after reauthentication. A process-level trace test confirms malformed payload content and synthetic credentials stay redacted while HTTP remains live. Formatting, strict Clippy and all 42 tests passed.

## TN-14 container cleanup results (2026-11-13)

The validator now prints its passing report after the cleanup context exits. Failed removals are checked against an exact-name Docker list query so a failed start with no residual container is distinguished from a daemon or removal failure. Every registered name receives a cleanup attempt. Cleanup failures make a successful validation fail; when validation already failed, its original exception survives and cleanup errors are reported on stderr.

Six Python regressions cover result ordering, a failed removal followed by another attempt, preservation of the primary exception, absent containers after failed starts, and Docker execution errors. They are included in scripts/check.sh. Formatting, strict Clippy, all 42 Rust tests and six Python tests passed. The linux/amd64 build and the actual local container suite passed with the revised cleanup path.

## TN-15 fixed listener port (2026-11-13)

The shared exporter listen-address parser rejects port zero with a setting-specific error. Parser cases cover IPv4/IPv6 wildcard and loopback addresses, the default port and valid boundary ports. Child-process tests verify both normal startup and the credential-free healthcheck reject zero before attempting service operation. Loopback HTTP tests verify IPv4 and IPv6 wildcard addresses still map to their respective loopback addresses for the healthcheck.

Formatting, strict Clippy, all 45 Rust tests and six Python cleanup tests passed. TN-02 remains pending captured, sanitized appliance response evidence; the capture directory still contains only its README.

## Operational acceptance continuation — November 13, 2026

The current tree passes Rust 1.88 formatting, strict Clippy and the locked 45-test suite, with one additional operational test explicitly ignored by default. Six Python cleanup tests also pass. The MSRV run found a Clippy simplification in the pool-array boundary check; the equivalent is_none_or expression now satisfies both installed stable and Rust 1.88.

application/tests/operational.rs runs the real exporter process with 16 independent synthetic WSS sessions, 256 pools per target and maximum-length pool labels. Four scrape clients run alongside actual polling for 30 seconds. The test interrupts all transports, verifies readiness failure and removal of expired pool measurements while retaining last-success metadata, restores the mock service, verifies all targets recover, and sends SIGTERM with a metrics client holding a 1 KiB receive window open.

The first native run with a five-second outage recorded 190 successful scrapes, 1,876 busy responses, 568 collection requests and 2,050 ms SIGTERM-to-exit. An initial limited-container run recorded 352 successful scrapes, 1,598 busy responses, 580 collection requests, 40 ms maximum health latency and 2,016 ms SIGTERM-to-exit. Its peak cgroup usage was 91,516,928 bytes. These initial runs used the former runtime. The final scenario extends the outage to fifteen seconds against the refreshed runtime.

The test is finite operational evidence for the current pool exporter. Appliance credentials/roles, native Ubuntu deployment and hosted GitHub CI remain separate acceptance work.

To reproduce natively from the repository root:

~~~sh
cargo +1.88.0 test --locked --manifest-path application/Cargo.toml --test operational sustained_collection_outage_recovery_and_stalled_sigterm -- --ignored --nocapture
~~~

To export the operational binaries and run with 128 MiB, no swap, 0.5 CPU, loopback-only networking, a read-only runtime and a temporary filesystem:

~~~sh
validation_dir=$(mktemp -d /tmp/truenas-operational.XXXXXX)
docker --context desktop-linux build --platform linux/amd64 -f deployment/Dockerfile -t local/truenas-rust-exporter:operational .
docker --context desktop-linux build --platform linux/amd64 -f deployment/Dockerfile --target operational-test --output "type=local,dest=$validation_dir" .
docker --context desktop-linux run --rm --platform linux/amd64 \
  --network none --memory 128m --memory-swap 128m --cpus 0.5 \
  --read-only --tmpfs /tmp:rw,size=16m --cap-drop ALL \
  --security-opt no-new-privileges:true \
  --mount "type=bind,src=$validation_dir,dst=/validation,readonly" \
  --mount "type=bind,src=$validation_dir/truenas-exporter,dst=/build/application/target/release/truenas-exporter,readonly" \
  --entrypoint /validation/operational \
  local/truenas-rust-exporter:operational sustained_collection_outage_recovery_and_stalled_sigterm --ignored --nocapture
~~~

The operational-test build stage exports test binaries only. The final runtime contains the exporter and license. Real-runtime hardening checks pass with the refreshed base. The exact final image and scanner results are recorded in security-validation.md.

The final refreshed-runtime run used a fifteen-second outage and completed in 64.69 seconds. It recorded 518 successful scrapes, 2,202 encoder-busy responses, 699 collection requests, 35 ms maximum health latency, 2,013 ms SIGTERM-to-exit and 87,453,696 bytes peak cgroup usage (83.4 MiB). All targets recovered, expired measurements were absent, and the child exited successfully while the stalled reader stayed open. The runtime high/critical image gate also passed with zero findings at those severities.

## TN-02 — captured appliance pool compatibility, November 13, 2026

Explicit operator authorization covered only auth.login_ex and pool.query against the configured WSS endpoint, from the Mac and local Docker Desktop. Both native and distroless linux/amd64 checks passed verified TLS, typed SUCCESS authentication, pool.query with empty parameters, and the existing collector schema. One pool was returned each time. The operator confirmed version 25.04.2.6 and Readonly Admin in the UI; neither backend version nor minimum privilege was independently queried.

The temporary probe reused repository configuration/TLS, protocol and pool parsing code. It used 1 MiB message/frame limits and a 30-second outer deadline, without saving raw or authentication responses. Full sanitized host/container envelopes, including ignored topology/scan fields, are checked in under fixtures/truenas-25.04.2.6/. The fixture README records transformations and observed capacities. A preliminary projected capture was replaced by the full sanitized structure before committing.

The container probe used UID/GID 10001:10001, read-only root, dropped capabilities, no-new-privileges, 128 MiB memory and a restricted secret volume mounted read-only. A local helper provisioned that volume via stdin, mode 0400; it was removed in cleanup. The Mac credential remained mode 0600. The probe did not exercise the live exporter supervisor, HTTP server, outage recovery or production deployment against the appliance.

Focused test `captured_appliance_pool_envelopes_replay_offline` passed. Each envelope is replayed over verified loopback TLS through the production Client with only request ID correlation changed. The test checks the original fragmentation string, exact independently recorded size/allocated/free bytes, health/warning, target/pool labels, status metadata, HTTP metrics and readiness, and absence of the deferred ambiguous scrub metric. Missing/null/error cases remain covered by synthetic tests rather than fabricated captures.

`sh scripts/check.sh` passed formatting, strict Clippy, 46 regular Rust tests (one operational test explicitly ignored by default), and six Python tests. The production linux/amd64 image build passed as local/truenas-rust-exporter:captured-pools using existing cached build layers. The preceding operational and security runs were not repeated because no production implementation or dependencies changed. Hosted CI was not run.

No additional appliance methods, infrastructure edits, live dashboard changes, pushes or deployment were performed. M3 can start offline; live system/version/realtime validation needs separately scoped authorization.

## M3-A — system preflight, November 14, 2026

Implemented an explicit check-system command backed by a closed system.info request and typed selected-field parsing. The source/role/units/registration mapping is pinned to official middleware tag TS-25.04.2.6, commit 244b717370fe35fb9eefc3096acfbc25f12b57b6. See system-contract.md. No live appliance call was made in this milestone; all new system fixtures and credentials are synthetic.

Focused loopback TLS and actual CLI tests pass for valid 25.04.2.x data, other release families, malformed fields, permission/method errors, unknown correlation IDs/notifications and a stalled reply. The fixed-output CLI test also sets RUST_LOG=trace and checks that username, key, hostname, serial marker and sensitive remote error detail do not appear. Mock mode is rejected before network access. Field tests cover preserved fractional uptime, bytes and load averages, missing/null optional values, required-field omissions, numeric/string boundaries and invalid host/version lengths/formats.

The first full sh scripts/check.sh run failed in the existing real_process_keeps_http_alive_redacts_trace_and_stops_on_sigterm test with exporter exited prematurely. No new preflight path is invoked by that test. An isolated rerun passed, followed by a clean full-suite rerun: formatting, strict Clippy, 52 regular Rust tests and six Python tests passed; the sustained operational test remains intentionally ignored by default. The initial early exit was not reproduced or diagnosed, so no unrelated implementation fix or definitive cause is claimed.

Rust 1.88 strict Clippy across all targets and all six new system/preflight tests passed. The linux/amd64 production image local/truenas-rust-exporter:system-preflight built successfully on local Docker Desktop. The standard check-container.py hardening/synthetic-secret/multi-target/outage/SIGTERM validation passed on that image. A network-disabled container smoke check confirmed check-system is advertised in help and rejects mock configuration. Live Docker system.info permission and response compatibility remain unverified.

No dependency/base-image changes were made, and prior security scans and sustained operational measurements were not rerun. Hosted CI was not run. Normal pool polling, cached metrics and readiness retain their existing behavior. Automatic runtime version gating and system metrics are not implemented by M3-A.

## M3-B — authorized system preflight and captured replay, November 14, 2026 UTC

The operator explicitly approved read-only auth.login_ex and system.info checks from the Mac and local Docker Desktop against wss://memory-alpha.home.lab/api/current with the existing dedicated read-only credential. The actual check-system command passed in both environments using verified TLS and its single API-timeout budget. It returned fixed success text without exposing raw fields. The container ran the production distroless linux/amd64 image as UID/GID 10001:10001.

A separate temporary capture harness then authenticated with auth.login_ex and requested system.info with empty parameters in each environment using the repository configuration/TLS/protocol/authentication helpers and system parser, 1 MiB frame/message limits and a 30-second overall deadline. It sanitized the complete successful response structure before writing. Raw replies and authentication exchanges were not saved. Full host/container fixtures and exact timestamps, transformations and measurement provenance are in fixtures/truenas-25.04.2.6/README.md.

Both system.info captures report version 25.04.2.6 and 63,093,960,704 physical-memory bytes. The observed uptime and three load averages are preserved independently for each capture. Their serial/model/manufacturer/product/timezone strings are replaced, hostname is aliased and license is null. The existing typed decoder passed unchanged.

The new captured_system_envelopes_replay_through_preflight_offline test replays both full envelopes over loopback TLS through production check_system, replacing only request correlation. It checks exact version, aliased hostname, memory bytes and load values, plus fractional uptime within one microsecond. It passed on the default toolchain and Rust 1.88.

sh scripts/check.sh passed formatting, strict Clippy, 53 regular Rust tests and six Python tests. The sustained operational test remains intentionally ignored by the default suite. The production linux/amd64 image build passed as local/truenas-rust-exporter:system-captured, reusing unchanged production build layers. No production source, dependencies or base images changed; the prior hardened-container, sustained operational and security results retain their original recorded scope and were not rerun. Hosted CI was not run.

The Docker preflight and capture used a read-only root filesystem, dropped capabilities, no-new-privileges, 128 MiB memory and a mode-0400 UID-10001 secret mounted read-only. The secret was copied over stdin into temporary volumes and removed in cleanup. Final container/volume inventories were empty for these temporary names; the original Mac key remains mode 0600.

This verifies successful system.info read permission for the dedicated credential and its current API-reported version. It does not establish minimum privileges, denial of writes, automatic per-connection version gating, scheduled system metrics, realtime subscriptions or production deployment. No method beyond auth.login_ex and system.info was invoked in this milestone.

## M3-C — opt-in system collection and connection gate, November 14, 2026 UTC

TRUENAS_SYSTEM_ENABLED=true enables system.info gating on every authenticated connection/reconnection, independent system/pool polling and required-group freshness. The default false retains the existing pool-only behavior. The new state shares typed snapshot/operational metric logic, not timestamps or payloads. Known incompatible/unvalidated versions reject the session and immediately suppress measurements/readiness for that target while retaining historical timestamps/errors. Only the connection gate can restore compatibility. Ordinary transport outages and collector field/permission failures retain bounded per-group cache grace.

Unit tests cover independent exact-boundary expiry/recovery, preserved error/last-success history, absent optional values, compatibility rejection and late completion, method-aware response routing, the 64 KiB system result limit and the existing 256-pool bound, and unknown IDs without result expansion. Loopback TLS tests cover out-of-order system/pool replies, system field errors without pool interruption, reconnection through supported/incompatible/supported versions, scheduled incompatible-version invalidation and cancellation during the connection gate.

A real exporter-process test uses two independent TLS targets with the saved system fixture. It verifies enabled-mode gate/poll/metrics wiring, target isolation and a fresh system snapshot while one target's pool snapshot expires. Existing default-mode credential/CA isolation, health, shutdown and trace-redaction checks also pass. Controlled-time unit tests cover the reciprocal case of system expiry with fresh pools.

Validation completed:

- sh scripts/check.sh: formatting, strict Clippy, 62 regular Rust tests and six Python tests passed. One sustained operational test remains explicitly ignored by default.
- cargo +1.88.0 clippy --locked --manifest-path application/Cargo.toml --all-targets -- -D warnings: passed.
- cargo +1.88.0 test --locked --manifest-path application/Cargo.toml: all 62 regular tests passed, one operational test ignored.
- docker --context desktop-linux build --platform linux/amd64 -f deployment/Dockerfile -t local/truenas-rust-exporter:system-collector .: passed, linux/amd64.
- python3 -B scripts/check-container.py --context desktop-linux --image local/truenas-rust-exporter:system-collector: passed default mock, system-enabled mock metrics/readiness, hardened UID/GID-10001 runtime, synthetic secret/CA mounts, multi-target/outage and SIGTERM checks. The enabled mock explicitly retains API connected/compatible zero and uses synthetic system data. No real appliance was contacted.

The first full required-suite run stopped on an existing configuration/process fixture's ConnectionReset while reading its local /healthz response. The full Rust 1.88 run and subsequent required-suite rerun passed. The intermittent reset was not diagnosed or fixed, and no unrelated production or test behavior was weakened to obtain a pass.

This milestone introduced no dependency/base-image changes. Prior security scans and sustained resource measurements retain their historical workloads; the maximum-target resource benchmark was not rerun with system mode enabled. Hosted CI and live scheduled-mode acceptance were not run. Earlier individually authorized pool/system checks and captured 25.04.2.6 replies remain the scope of appliance evidence. No new live method, subscription, infrastructure edit or production deployment occurred.

Compose configuration checks passed for the local mock project and for the observability integration fragment combined with a temporary synthetic network definition. The fragment intentionally requires its enclosing project to define that network; it is not a standalone deployable project. No services or networks were created by these parser checks.

## M3-D — opt-in realtime implementation, November 14, 2026 UTC

Implemented a separate authenticated/version-checked realtime socket per enabled target, with exact reporting.realtime subscription selection and connection-owned unsubscribe. TRUENAS_REALTIME_ENABLED defaults false and requires system collection. CPU, physical memory, network and ARC use independent typed latest snapshots and realtime TTLs. All enabled groups are required for readiness; no sibling update refreshes a failed group. Failed backend/partial fields never fabricate zeros. Down-link rates/speed are omitted; memory total remains the system.info family to avoid duplicate sources. Source mappings, upstream zero-default limitations and cleanup bounds are recorded in realtime-contract.md.

The one-shot check-realtime authenticates, reads system.info, subscribes to this event, observes fresh groups, unsubscribes the acknowledged identifier and closes with fixed redacted output. It sends no pool query. It is implemented for future separately authorized native/container acceptance; it was not run against MEMORY ALPHA. No role or captured realtime compatibility claim is made.

Offline verification:

- sh scripts/check.sh passed formatting, strict Clippy, 76 regular Rust tests and six Python tests. One sustained operational test remains explicitly ignored by default. Final full checks passed after the last source-neutral HELP wording refinement.
- Full Rust 1.88 strict Clippy and cargo test --locked passed all 76 regular tests; one operational test ignored.
- Schema tests cover byte/percent/Mbps conversions exactly once, up/down/missing network fields, backend-only failure frames, sibling-group isolation, invalid types/ranges, bounded CPU/interface maps and duplicate keys, optional ARC ratios, independent exact expiry/recovery and deletion of empty interface snapshots.
- Protocol/TLS tests cover foreign collection filtering, control/text bounds, pre-ACK latest replacement and original receipt age, permission retries, notify_unsubscribed recovery with new owned IDs, silence-triggered reconnect while pongs continue, cleanup ownership and a one-second missing-ACK bound.
- The actual check-realtime CLI passed loopback TLS with RUST_LOG=trace and returned no credentials, source payload, hostname or subscription identifier.
- A real daemon-process test accepted exactly two TLS sockets, served combined polling/realtime metrics, checked no duplicate total-memory family or down-link receive sample, retained redacted trace logs and completed owned unsubscribe/SIGTERM shutdown. Existing polling-only and system-mode tests remain green.
- The final linux/amd64 image local/truenas-rust-exporter:realtime built successfully and passed scripts/check-container.py with hardened default and system/realtime-enabled synthetic profiles. Final image ID: sha256:e5922d872114bd0e0c8717e6ececb7d157836f1c63740783876121aaee26d504. The enabled mock exposes fresh CPU/memory/network/ARC while API connected/compatible/subscribed remain zero. No appliance traffic occurred.
- Compose configuration checks passed for the mock project and the integration fragment combined with the temporary synthetic enclosing network definition, without starting services or creating infrastructure.

Limits: no live core.subscribe/core.unsubscribe, no captured realtime response, no observed REPORTING_READ permission, no UI/Graphite equivalence check and no integrated production deployment. Maximum-target CPU/RSS/outage measurements have not been repeated with both sockets/realtime groups enabled. Prior security scans retain their original scope; dependencies/base images were not changed. Hosted CI was not run. Earlier intermittent process-fixture failures remain historical unresolved reliability work; the final M3-D full runs passed without those failures.

## Authorized native/container realtime preflight — November 14, 2026 UTC

The operator explicitly authorized check-realtime against wss://memory-alpha.home.lab/api/current from macOS and local Docker Desktop using the existing dedicated credential file. Both commands exited successfully, reporting verified WSS/system compatibility, fresh CPU/memory/network/ARC groups and closed owned subscription. Native interval configuration was confirmed to use the authorized two-second setting; the Docker command used the default two-second interval.

The calls were auth.login_ex, system.info, core.subscribe for exactly reporting.realtime:{"interval":2}, then core.unsubscribe for that connection's acknowledged identifier. A successful command includes a null unsubscribe result and successful WebSocket close. It sends no pool request or administrative method. This demonstrates effective event-read/unsubscribe permission and acceptance of the selected live schemas, not minimum privileges, exact role inventory or numerical UI/Graphite equivalence.

Docker used desktop-linux, linux/amd64, local/truenas-rust-exporter:realtime image ID sha256:e5922d872114bd0e0c8717e6ececb7d157836f1c63740783876121aaee26d504, UID/GID 10001:10001, read-only root, dropped capabilities, no-new-privileges and a 128 MiB limit. The key was transferred over stdin into a temporary volume, mode 0400/owner 10001, and mounted read-only alongside the read-only CA. All temporary secret volumes and helper/probe containers were removed; subsequent inventories were empty. The original Mac key remains mode 0600.

No authentication frame, source identity, raw response, event payload or subscription ID was printed or saved. The command does not create a captured fixture. Existing synthetic realtime tests remain explicitly synthetic; earlier pool/system fixtures retain their original capture provenance and version observation.

Before the evidence commit, sh scripts/check.sh passed formatting, strict Clippy, all 76 regular Rust tests and six Python tests (one sustained operational test intentionally ignored). The production linux/amd64 image build also passed using unchanged production build layers. Production source, dependencies and base images did not change; the previous full Rust 1.88 and hardened-container results retain their recorded scope and were not rerun. Hosted CI was not run.

Remaining acceptance: reviewed sanitized realtime captures and offline replay, observed alternate/down/missing cases where available, numerical comparison, longer-running integrated daemon/outage tests and maximum-target resource measurements. No continuous daemon or production deployment was started, and no infrastructure/dashboard configuration changed.

## Authorized realtime capture and offline replay — November 14, 2026 UTC

Continued operator authorization covered the same read-only auth.login_ex, system.info, exact reporting.realtime:{"interval":2} core.subscribe and connection-owned core.unsubscribe workflow from macOS and local Docker Desktop. Two notifications were collected in each environment. Both connections reported version exactly 25.04.2.6 and acknowledged owned unsubscribe before saving. No additional method, infrastructure edit or production deployment occurred.

A temporary source-copy harness reused production connection/parsing/cleanup code. It sanitized event text in memory before writing: fixed schema keys were checked, interface keys consistently aliased, other non-protocol/non-link-state strings replaced, and numeric/bool/null values and structure retained. Raw authentication/system responses, subscription IDs and interface identity mappings were not persisted. Full sanitized envelopes, receipt-time/version/cleanup provenance and independently reviewed selected wire values are recorded in fixtures/truenas-25.04.2.6/README.md. Native receipts were 05:57:23/25 UTC; container receipts were 06:00:13/15 UTC. These local receipt times do not verify appliance clock accuracy.

The new captured_realtime_notifications_replay_through_worker_and_metrics test sends all four complete notifications unchanged over loopback TLS through the production realtime worker and metric encoder. It checks aggregate CPU conversion and 13 CPU series, memory/ARC bytes, up-link state and Mbps conversion, byte rates, explicit ARC zeros, target/interface labels, absence of duplicate total-memory samples and owned cleanup. Authentication/version/ACK replies and inspecting state are synthetic; this test does not establish integrated daemon readiness against the appliance.

Validation passed: sh scripts/check.sh (formatting, strict Clippy, all 77 regular Rust tests and six Python tests; one sustained operational test intentionally ignored), the new replay under Rust 1.88 via cargo +1.88.0 test --locked --manifest-path application/Cargo.toml --test api_client captured_realtime_notifications_replay_through_worker_and_metrics, and the production image build tagged local/truenas-rust-exporter:realtime-captured. Production source, dependencies and base images did not change. Previous full MSRV, hardened-container, security and resource results retain their recorded scope and were not repeated; hosted CI was not run.

Docker capture used UID/GID 10001:10001, read-only root, dropped capabilities, no-new-privileges and a 128 MiB limit. Its read-only mode-0400 secret was transferred over stdin into a temporary volume. Final inventories confirmed temporary secret volumes/helper/probe containers removed; the Mac key remained mode 0600. A configured resource limit is not measured maximum-target resource acceptance.

All four frames have one up interface. Down/missing/backend-failure/null-temperature variants remain explicitly synthetic, not captured. Selected ARC hit percentages are zero in every capture; pinned upstream code can default missing dimensions to zero. Faithful replay therefore does not establish cache efficiency or independent chart availability. Sequential native/container CPU, memory and rates need not match. Numerical/source-chart comparison and integrated two-socket runtime/resource/outage acceptance remain open; no appliance faults were induced to obtain alternate fixtures.

## Numerical source review — November 14, 2026

Reviewed additional public pinned chart/counter producers and the existing event adapters; verified all 20 locally reviewed files against their Git blob identities at middleware commit 244b717370fe35fb9eefc3096acfbc25f12b57b6. An isolated temporary Python harness exercised pure upstream conversion functions with constructed dictionaries/counters, without procfs/Netdata/service execution. Deferred type annotations were required for Python 3.9; the initial unadapted import failed, then the complete adapted experiment passed. Results and limits are in realtime-numerical-review.md. No appliance API, UI, Graphite or runtime-engine comparison occurred.

Confirmed source arithmetic: CPU excludes idle/iowait; memory bytes originate from kB x1024; network adapter applies absolute value and rounding after kilobit/s-to-byte/s conversion; selected ARC percentages round to integers and default missing/null to zero. Missing-data defaults also affect other groups. The ARC utility calculates ordinary-hit percentages from cumulative counters, while the chart registers percentage-of-incremental-row. This is an unresolved source semantic concern, not a reproduced engine defect or a diagnosis of captured zeros. No exporter behavior was changed and independent numerical acceptance remains open.

sh scripts/check.sh passed formatting, strict Clippy, 77 regular Rust tests and six Python tests, with one sustained operational test intentionally ignored. The first sandboxed run failed because two local shutdown-test listener binds returned Operation not permitted; rerunning with local loopback permission passed the complete suite. The production linux/amd64 image build tagged local/truenas-rust-exporter:numerical-review passed. Production source/dependencies/base images are unchanged; no fresh full MSRV, security, hardened-container or resource result is claimed. Hosted CI was not run.


## Integrated system/realtime finite operational acceptance — November 14, 2026

The new explicitly ignored integrated scenario in application/tests/operational.rs passed on native arm64 macOS and network-disabled linux/amd64 Docker Desktop. It uses 16 targets/32 WSS sessions, 256 pools, 257 CPU entries and 128 interfaces per target; four scrape clients, realtime-only silence with continuing polling/pongs, full outage, recovery and stalled-reader SIGTERM. Required expiry/readiness/isolation checks passed; the fixture verified 16 final owned unsubscribe requests and all server sessions drained. No appliance traffic or production code/configuration change occurred.

Native Rust 1.88 release: 74.44 seconds, 883 successful scrapes, 1459 expected busy responses, 760 pool requests, 1 ms maximum sampled health latency and 2003 ms shutdown. External 500 ms child sampling observed a 489,865,216-byte RSS peak and 41.19% average of one core over 74.097 seconds. Final instrumented Docker Rust 1.88 release: 74.91 seconds, 253 successful scrapes, 1875 expected busy responses, 778 pool requests, 40 ms maximum sampled health latency and 2022 ms shutdown. Exporter child sampled RSS peaked at 62,930,944 bytes, with 19.03 CPU seconds over 72.455 seconds (26.26% of one core). Whole-cgroup memory peaked at 126,873,600 bytes under 128 MiB; this includes fixture/client overhead and is not child RSS. The earlier non-instrumented container pass also succeeded but peaked at 130,547,712 bytes. Run variation is not an optimization claim.

The original pool-only sustained native regression also passed after the shared-fixture refactoring. Final sh scripts/check.sh passed formatting, strict Clippy, 77 regular Rust tests and six Python tests; two sustained tests are now explicitly ignored by default and were run separately. Native Rust 1.88 all-target strict Clippy passed, and the updated Linux operational harness compiled and ran on pinned Rust 1.88. The production container build passed; its source, dependencies and base pins are unchanged. Hosted CI and fresh security scans were not run.

See integrated-operational-validation.md for exact workload, reproducible commands, image/binary identities and measurement limits. This closes the finite synthetic integrated recovery/shutdown gate, not long-duration stability or a universal memory budget. Native RSS and small capped-container margins require a longer steady-state/recovery profile before making low-memory/headroom claims. Independent live numerical/chart verification, least privilege and deployment remain separate gates. No production configuration, dashboard, infrastructure, credential, push or tag changed.


## Phased memory and repeated recovery — November 14, 2026

The new explicitly ignored integrated_memory_profile passed on native arm64 macOS (328.72 seconds) and network-disabled linux/amd64 Docker Desktop (330.50 seconds), using Rust 1.88 release. It shares the 16-target/32-socket maximum-cardinality fixture, adding a warmed 30-second baseline, 120 seconds of four-client scrape pressure, 45-second cooldown and three eight-second outages with recovery/quiet periods. Every cycle verified expired measurement removal, readiness recovery and exactly 32 active sessions; shutdown verified 16 owned requests and complete session drainage. Test-only macOS ps and Linux procfs sampling now produce per-phase child CPU/RSS JSON. No production source/default/resource configuration or appliance access changed.

Native sampled RSS peaked at 471.39 MiB during scrape load, ended cooldown at 53.41 MiB and had quiet recovery minima of 53.61/53.64/53.67 MiB. It remained about 13.9 MiB above the warmed baseline end; source-level ownership or allocator cause was not established. Linux child RSS peaked at 60.55 MiB, ended cooldown at 47.82 MiB and had quiet recovery minima around 41.75/41.76/41.77 MiB. The final phase's ending 54.52 MiB includes final metric/stalled-reader setup. These finite, coarse summaries do not prove an indefinite plateau or allocation-level leak freedom.

The whole Docker cgroup reached its exact 128 MiB cap while both processes completed successfully. Fixture/client/cache charges share this accounting; memory.events was not recorded, so peak alone does not characterize pressure/reclaim/OOM-event history. Production headroom remains open pending separate exporter/fixture accounting. No allocator or production limit was changed. Full phase values, workload, actual runtime/binary identities and instrumented-source hash are in operational-memory-profile.md and evidence/operational-memory-profile-2026-11-14.json.

Final sh scripts/check.sh passed formatting, strict Clippy, 77 regular Rust tests and six Python tests; three sustained scenarios are now intentionally ignored by default. This milestone separately executed the new profile on both platforms; the two older sustained scenarios retain the preceding milestone's results. The first ordinary-suite run failed in the pre-existing configuration process test environment_unicode_errors_are_redacted_and_unrelated_bytes_are_ignored with ConnectionReset at tests/config.rs:316. Its isolated rerun and full-suite rerun passed; the cause remains unresolved, and this is not claimed as a fixed regression.

Native Rust 1.88 all-target strict Clippy passed. The Linux operational target also passed strict Clippy under pinned Rust 1.88 after adding that component only to a temporary validation build stage. Initial Linux-lint setup failed on unrelated temporary-directory metadata, then on the missing builder component; neither was a source diagnostic. The production container build passed; its source/dependencies/base pins are unchanged. Hosted CI, new security scans and full ordinary-suite MSRV tests were not repeated. Temporary profile containers were removed; no push, tag, production deployment or infrastructure/dashboard change occurred.


## Separate exporter/fixture cgroup profile — November 14, 2026

The new controller-driven ignored scenario passed the same finite phased workload in 328.26 seconds. The actual production image entrypoint ran in its own UID-10001, read-only, no-capabilities, no-new-privileges cgroup at 128 MiB/no swap/half a CPU; the fixture had a separate 256 MiB/no swap/one-core cgroup. They shared only the fixture's network-none loopback namespace. Native external Docker stats observed 323 paired samples; no observer process or Docker socket entered either container. Normal exporter image healthchecks remained enabled.

All expiry/recovery/session/owned-shutdown assertions passed: 840 successful scrapes, 6100 expected encoder-busy responses, 4384 pool requests, 45 ms maximum sampled health latency, 32 active sockets after each recovery and 16 final owned unsubscribe requests. Controller-coordinated stop took 2189 ms including polling/API overhead; it is not an isolated SIGTERM latency. Both containers exited zero, exporter OOMKilled=false, and owned containers/control files were removed. Only generated synthetic credentials/CA/configuration were used. No appliance, Prometheus/Grafana, production source, deployment or infrastructure change occurred.

Maximum sampled exporter cgroup charge was 63.30 MiB under 128 MiB, versus fixture 53.42 MiB under 256 MiB. Exporter sampled margin was 64.70 MiB; true kernel high-water and memory-event history were not available from Docker stats. The fixture independently printed a 54.84 MiB kernel peak, showing the limitations of sampling. No universal production budget or optimization is claimed. Exact workload, limits, source/image identities, phase summaries and selected time series are in isolated-cgroup-validation.md and evidence/isolated-cgroup-profile-2026-11-14.json.

Final sh scripts/check.sh passed formatting, strict Clippy, all 77 regular Rust tests and eleven Python tests (six existing cleanup cases plus five new controller-boundary cases). Four sustained scenarios are now ignored by default. The isolated profile ran under pinned Linux Rust 1.88 release; the existing local-child integrated scenario also passed natively on Rust 1.88 after the launch refactoring. Native Rust 1.88 all-target strict Clippy and Linux operational-target strict Clippy passed. The production image build passed; dependencies/base pins are unchanged. Full ordinary-suite MSRV tests, hosted CI and new security scans were not repeated. Historical intermittent configuration/process-test failures retain their unresolved status; the final ordinary suite passed on its first run this milestone.


## Documentation and conversation handoff checkpoint — November 14, 2026

Documentation-only release 0.3.7 reconciles current status against functional/tooling baseline 069450d (0.3.6). Added project-status.md, a current implementation plan, an archived prior plan, comprehensive conversation handoff and pasteable restart prompt. Updated current-facing API/architecture/configuration/metric/compatibility/operations summaries; preserved dated capture, operational and security evidence with explicit scope/navigation. External action-plan indexes are reconciled, prior narratives archived and handoff/prompt mirrored there. No production source, tests, runtime configuration, dependencies or base pins changed.

sh scripts/check.sh passed formatting, strict Clippy, 77 regular Rust tests and 11 Python tests; four sustained tests remain ignored by default. The linux/amd64 production build tagged local/truenas-rust-exporter:handoff passed using unchanged production layers. Local Markdown targets across 37 repository/external documentation files were checked with zero missing links. Live appliance calls, sustained resource reruns, full MSRV-suite reruns, hosted CI and new security scans were not performed for this documentation checkpoint. Existing evidence retains its recorded scope.

The next conversation should read project-status.md, implementation-plan.md and handoffs/2026-11-14-handoff.md, then begin offline callable SCRUB/RESILVER contract work. No push, tag, production deployment, dashboard/infrastructure/access-control change or credential-content read occurred.

## November 14, 2026 — 0.4.0 pool scan metrics

Both sanitized pool responses replayed through the production client and HTTP metrics endpoint, exposing FINISHED SCRUB, errors=0 and the source percentage divided by 100. Synthetic tests covered active and canceled RESILVER, unsupported/missing/null scan identity, invalid numeric values, pool replacement, failed-poll retention and expiry at the TTL boundary.

`sh scripts/check.sh` passed formatting, locked all-target Clippy with warnings denied, 79 ordinary Rust tests and 11 Python tests. The initial sandbox run denied loopback listeners. The complete run with loopback access passed. Four sustained scenarios were ignored as configured. The pinned linux/amd64 runtime built on Docker Desktop context desktop-linux. Verification used public release sources and local fixtures. Hosted CI, sustained profiling and live acceptance were outside this run.

## November 14 — numerical reverification, 0.4.1

Fresh public pinned source review and pure CPU/normalization/missing-dimension experiments passed. Existing four-frame worker replay passed in the full suite. Initial relocated build artifacts referred to the former checkout path and caused three process spawn NotFound errors. Rebuilding the package cache resolved these failures. This does not establish the root cause of the earlier intermittent ConnectionReset issue. Full scripts/check.sh then passed 79 Rust and 11 Python tests, formatting and strict Clippy. Docker Desktop was started locally and the linux/amd64 image build passed. No live observation was executed. Exact bounded proposals are in numerical-live-comparison.md.

## November 14 — release identity and local candidate checks, 0.4.2

The executable now embeds root VERSION, including the Docker build. A configuration-free CLI regression verifies the release text. Ordinary checks passed 80 Rust and 11 Python tests on stable and Rust 1.88, including strict Clippy and formatting. Four sustained tests remained ignored. The desktop-linux linux/amd64 image built successfully and its --version returned 0.4.2 with networking disabled. Existing check-container.py passed synthetic mounts, freshness/outage, multi-target, mock realtime and hardening checks. cargo audit --deny warnings passed. Trivy's HIGH,CRITICAL gate passed for 14 Debian packages; its output reported no language-specific files, so Rust coverage comes from cargo-audit. Hosted CI remains unexecuted.

## November 14 — authorized selected event window, 0.4.3

One approved native event window completed with 45 selected observations, version exactly 25.04.2.6, acknowledged owned unsubscribe and socket close. Last observation arrived 89.352 seconds after harness start. Saved evidence contains converted selected measurements and aliases. No paired Reporting or raw-counter evidence was acquired.

The final ordinary suite passed 80 Rust tests and 11 Python tests, formatting and strict Clippy. Twenty additional config-suite repetitions passed (220 test executions). The historical intermittent ConnectionReset/early-exit cause was not reproduced or closed. Sustained profiles and hosted CI remained outside this run.

The pinned linux/amd64 container build for 0.4.3 passed on desktop-linux. Fresh security/hardening evidence above belongs to the 0.4.2 image. This release changes the embedded release text and documentation/evidence; it does not claim a fresh security scan for the resulting image identity.

## November 14 — supported reporting comparison plan, 0.4.4

Offline review verified 35 reporting implementation/schema files against pinned Git blob IDs, including the current v25_04_2 schema. An AST check confirmed all eight proposed ARC plugin/chart mappings. Public chart history is available under REPORTING_READ; system.info declares READONLY_ADMIN. Public history null2zero and the batch literal schema limit the evidence it can provide. The new plan specifies exact public requests and separate operator-local raw-counter/metadata scope. No appliance access was performed for this milestone. scripts/check.sh passed formatting, strict Clippy, 80 ordinary Rust tests and 11 Python tests. The desktop-linux linux/amd64 container build passed. Four sustained cases remained ignored.

## November 14 — bounded public chart harness, 0.5.0

The explicit example and dedicated diagnostic module were tested against synthetic verified loopback TLS. Three focused tests cover the full 13-call sequence, sanitization/null preservation, invalid-event owned cleanup and chart bounds. The first operator-approved live attempt collected 30 selected events and stopped at RPC four during unsubscribe handling, before chart requests. It preserved sanitized partial evidence and sent a close frame. It did not observe the unsubscribe acknowledgment. Pinned source supports a termination-notification-before-reply sequence that the harness initially rejected. The corrected fixture sends that notification, and both success and error-cleanup regressions pass. No automatic live retry was performed.

Final scripts/check.sh passed fmt, strict all-target Clippy, 83 ordinary Rust tests and 11 Python tests. Rust 1.88 all-target strict Clippy and the desktop-linux linux/amd64 image build passed. Four sustained cases remained ignored. A corrected additional live window is awaiting explicit authorization. Public chart numerical acceptance remains incomplete.

## November 14 — separately authorized corrected public chart window, 0.5.1

The corrected window completed 13 RPC requests in 59.072 seconds, with 30 events, 12 validated selected chart histories, 20,326 chart-reply bytes, acknowledged owned unsubscribe and socket close. Both ARC percentage histories returned zero alongside positive hit-rate histories. Receipt-aligned residuals are documented without claiming independently paired samples or raw-counter accuracy. Sanitized selected evidence and residual summaries are committed. No additional appliance window was attempted.

Final scripts/check.sh passed formatting, strict all-target Clippy, 83 Rust tests and 11 Python tests. The pinned desktop-linux linux/amd64 image build passed. Four sustained tests remained ignored. Raw counter/metadata, combined scheduled daemon, minimum roles and hosted CI remain open.

## November 14 — complete dashboard and authorized Mac stack, 0.6.0

The operator authorized upgrading local Grafana and running the exporter alongside Prometheus in the metric-simulator-observability Docker Desktop project. A verified mode-0600 backup of stopped Grafana data/provisioning/Compose preceded the upgrade. Local /api/health reported 13.2.2 with commit 1bea008f7e4e858b6824c9e364d608bd4d10b13a. No Linux production configuration was changed.

The separate hardened exporter container uses the existing dedicated credential through a protected mode-0400 volume, and a read-only public CA mount. It performs normal pool/system polling and realtime subscription. Prometheus configuration was backed up, passed promtool validation and was reloaded with the local exporter scrape job. All six collectors became successful, readiness passed and Prometheus up was 1 before the release-image update. Startup can record unavailable attempts before connection admission and readiness recovers on the following 60-second poll. This is recorded operational behavior rather than a closed reliability finding.

The dashboard covers every one of the 34 cataloged metric families plus scrape up across 34 measurement panels and six row headers. All 33 currently emitted families were covered; the polling connection error family was absent before its first error. All panel expressions executed without PromQL errors against local Prometheus. The Grafana resource database confirmed all 40 panel/row entries and seven variables were persisted. Browser visual inspection was unavailable. The scan progress display uses six decimal places in percent units to retain finished values below 100.

scripts/check.sh passed formatting, strict Clippy, 83 Rust tests and 14 Python tests. Three new dashboard tests cover metric completeness, non-overlapping layout and unit semantics. The pinned linux/amd64 0.6.0 image built successfully. Four sustained cases remained ignored. The service was updated to the reviewed release image within the existing authorized ongoing local scope.

Final post-update verification confirmed exporter readyz=200, minimum collector success=1 across all six groups, healthy Grafana/Prometheus/exporter containers, credential-copy permissions 0400 with owner 10001:10001, all dashboard queries successful and the final JSON persisted in Grafana. The live service runs local/truenas-rust-exporter:0.6.0.
