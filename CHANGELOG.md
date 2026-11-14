# Changelog

## 0.4.3 - 2026-11-14

- Record the authorized 45-event numerical observation with sanitized selected values and successful subscription cleanup.
- Preserve the remaining independent numerical and ARC zero-provenance acceptance gaps.


## 0.4.2 - 2026-11-14

- Report root VERSION from the executable and include it in container builds.
- Require an explicit reviewed image for the production Compose example; use a development tag for synthetic local builds.


## 0.4.1 - 2026-11-14

- Reverify captured CPU, memory, network and ARC conversions against pinned source and specify bounded live comparison windows.
- Include the operator copyright correction and updated commit workflow.


Entries below describe their milestone at the time, including then-pending work. For current disposition see [project-status.md](docs/project-status.md).

## Development history

- 0.4.0 adds pool scan errors, reported progress and state with pool snapshot freshness. Both saved FINISHED SCRUB captures replay. RESILVER coverage uses the pinned producer and synthetic tests.

- Consolidate current implementation/acceptance status, archive superseded plan snapshots, reconcile stale summaries and add a comprehensive conversation handoff plus pasteable restart prompt. Preserve dated evidence and live authorization limits.

- Add an opt-in two-container cgroup profile with loopback-only synthetic fixtures, a separately capped production exporter, bounded read-only Docker statistics, controller guards and owned cleanup. Record finite sampled headroom separately from fixture cost and kernel-peak/production-budget limits.

- Add an opt-in phased memory profile with cross-platform child CPU/RSS sampling, baseline/load/cooldown and three recovery cycles. Preserve synthetic evidence and separate scrape-associated native peaks from quiet recovery and whole-cgroup pressure; no production behavior changes.

- Add an opt-in full-process maximum-cardinality system/realtime operational scenario, shared with the existing pool-only fixture. Verify silence isolation, full outage recovery, final owned unsubscribe requests and stalled-reader shutdown; record native and capped-container child/cgroup resource observations without claiming long-term memory stability.

- Trace pinned CPU/memory/ARC chart producers and realtime adapter arithmetic; verify synthetic conversion/default cases and document the unresolved ARC chart-algorithm concern and a concrete independent-comparison procedure. No exporter behavior changes.

- Add four sanitized native/container TrueNAS 25.04.2.6 realtime notifications, version/receipt/cleanup provenance and production-worker metric replay. Preserve explicit zero ARC values and document their upstream-default ambiguity; numerical and sustained integrated runtime acceptance remain open.

- Record successful operator-authorized native and UID-10001 Docker realtime preflights, including selected live group validation and owned unsubscribe acknowledgement. Preserve the distinction from captured fixtures, numerical comparisons and integrated runtime/resource acceptance.

- Implement opt-in M3-D realtime CPU/memory/network/ARC collection on an isolated authenticated/version-checked socket, with owned subscriptions, bounded latest snapshots, independent freshness, silence/reconnect recovery, bounded cleanup and a redacted check-realtime preflight. Verify with synthetic unit/TLS/process/container fixtures; live realtime acceptance remains pending.

- Add opt-in M3-C system collection: per-connection/reconnect compatibility checks, independently fresh system/pool snapshots, system identity/uptime/memory/load metrics, method-aware result bounds and required-group readiness. Preserve pool-only defaults and keep realtime disabled. Verify with unit, loopback TLS, two-target process, Rust 1.88 and hardened container mock checks.

- Complete M3-B with operator-authorized native and UID-10001 Docker system.info preflights, API-reported 25.04.2.6 version evidence, sanitized full system captures and offline production-preflight replay. Automatic system polling/version gating remains pending.

- Add M3-A check-system: a one-shot verified-WSS authenticated system.info preflight with typed version/identity/uptime/memory/load validation, one overall timeout, fixed redacted output and offline TLS/CLI tests. Automatic system polling and daemon version gating remain pending.

- Resolve TN-02 with operator-authorized native and UID-10001 Docker authentication/pool checks, sanitized full response captures, provenance and offline production-client/HTTP replay. Backend version remains operator-confirmed; other methods/events are unverified.

- Adopt the MIT license, Copyright (c) 2026 dodson Software ( dodson labs ).
- Validate the Rust 1.88 MSRV and remove the unmaintained rustls-pemfile wrapper in favor of rustls-pki-types PEM parsing.
- Add a strict RustSec audit CI gate and reserve fixtures/truenas-25.04.2.6/ for captured sanitized evidence.

- Initial M1/M2 implementation: offline mock mode, exporter endpoints, bounded pool snapshots, verified TLS JSON-RPC client and local test fixtures.
- Document API assumptions, freshness semantics, deployment boundaries and deferred realtime/system collectors.
- Pass local Rust 1.88, linux/amd64 image build and hardened container checks. Captured appliance fixtures remain outstanding; runtime high/critical findings are resolved by the distroless change.
- Digest-pin both container bases and include the MIT license in the runtime image.

- Replace Debian slim with signature-verified, digest-pinned distroless cc-debian13 while retaining UID/GID 10001 and the executable healthcheck.
- Replace shell-based container checks with a test-only mounted probe; cover synthetic secret/CA mounts and network-isolated outage readiness.
- Reduce detected OS packages from 91 to 14 and high/critical scan findings to zero. Retain 23 medium/8 low findings in the security report and add a pinned Trivy CI gate.

- Add explicit verified WSS transport diagnostics without credential loading or API calls; validate the MEMORY ALPHA Caddy endpoint from host and container.
- Support multiple independently configured TrueNAS targets with file-only credentials, per-target CA trust, isolated collection/freshness and stable target labels.
- Add offline multi-target process tests and read-only target/CA/credential mount examples. Authenticated live validation remains blocked on operator provisioning.
