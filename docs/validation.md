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
