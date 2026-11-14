# Local validation record

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

Latest offline suite: **22 tests passed**, 0 failed (10 API, 5 configuration, 1 multi-target process, 1 process/redaction, 5 state/HTTP tests). Both Rust 1.88 and installed stable strict Clippy checks pass; formatting includes the standalone runtime probe.

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
