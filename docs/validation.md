# Local validation record

## Current review status — November 13, 2026

Fourteen findings have implementation fixes. TN-02 now has authorized native/container pool captures and offline replay through the production client and HTTP metrics/readiness. Version 25.04.2.6 was initially operator-confirmed; the later M3-B system captures also report it through the API. Earlier results below retain their original scope.

| Finding | Commit | Disposition |
| --- | --- | --- |
| TN-01 | 4be109e | Implemented and locally verified |
| TN-02 | this milestone | Captured full sanitized responses; offline replay passed |
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
cargo +1.88.0 test --locked --manifest-path application/Cargo.toml --test operational -- --ignored --nocapture
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
  local/truenas-rust-exporter:operational --ignored --nocapture
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
