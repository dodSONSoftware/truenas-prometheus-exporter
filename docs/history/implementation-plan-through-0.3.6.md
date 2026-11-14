# Historical implementation-plan snapshots through 0.3.6

Archived November 14, 2026. The original sequential plan below contains superseded pending statements. Use the [current implementation plan](../implementation-plan.md) and [project status](../project-status.md) for present decisions. This archive preserves the earlier milestone narrative; it is not a current checklist.

# Implementation plan and acceptance checklist

## Current review status — November 13, 2026

Fourteen findings have implementation fixes. TN-02 now has sanitized full pool response captures, dedicated-account native/container authentication and pool checks, and offline replay coverage. The operator confirmed version 25.04.2.6; M3-B now also records that version from system.info. M3-C now implements opt-in automatic version gating and independently fresh system metrics; integrated live polling remains unverified. Earlier results below retain their original scope.

| Finding | Commit | Disposition |
| --- | --- | --- |
| TN-01 | 4be109e | Implemented and locally verified |
| TN-02 | this milestone | Captured native/container responses and offline production-client replay |
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

The operational continuation verifies sustained independent TLS connections, outage recovery, full-process shutdown under backpressure, Rust 1.88 and refreshed security gates. Hosted CI requires an authorized GitHub run. Authenticated pool compatibility passed. Wider appliance methods, realtime and eventual deployment acceptance remain open.

Accepted findings: missing captured fixtures; pin API evidence; enumerate protocol allowlist; resolve scan names; deterministic readiness/partial failure; complete source mappings; use one root repository. Their contracts are in compatibility.md, api-contract.md, architecture.md and metric-catalog.md.

## M1
- [x] Single root repository and one Rust package, fail-fast typed config.
- [x] HTTP operational metrics, liveness/readiness, explicit offline mock mode.
- [x] Deterministic pool snapshot failure/expiry state and tests.
- [x] Docker/Compose examples, root docs, configuration inventory.
- [x] Local fmt, strict Clippy, 45 Rust tests, six Python tests and process checks through TN-15.
- [x] linux/amd64 Docker build, mock Compose configuration and hardened local runtime checks. CI workflow supplied but not executed.

## M2 (only after API contract documentation)
- [x] Document API wire allowlist, auth, limits and compatibility assumptions.
- [x] Verified TLS/private CA and typed auth/pool query.
- [x] Bounded request correlation, deadlines, reconnect and cancellation.
- [x] Synthetic pool fixture and local WebSocket failure tests.
- [x] Captured sanitized pool.query envelopes with operator-confirmed 25.04.2.6 provenance and offline replay.

## M3 and later
- [x] Exact system/version/uptime/memory/load sources and explicit M3-A compatibility preflight.
- [x] Opt-in automatic per-connection runtime compatibility gate and scheduled system collector, with independent required-group freshness.
- [x] Source-pinned realtime subscription/recovery, independent group validation and silence expiry with synthetic wire fixtures.
- [x] Operator-authorized native and local Docker realtime preflight, including owned unsubscribe acknowledgement.
- [x] Capture four sanitized native/container realtime notifications with provenance and production worker/metric replay.
- [ ] General scan metrics (SCRUB and RESILVER), verified callable schema.
- [ ] Container outage/recovery, least privilege and runtime resource validation.
- [ ] P1 collectors only after P0 contracts are established.
- [ ] Separate operator-authorized appliance validation and production integration.

No implementation milestone is fully validated until its container checks pass. Synthetic tests are not appliance compatibility evidence. Milestone commits are authorized by the current user instruction and follow git-commit-message-instructions.md. Pushing and production rollout require separate authorization.

## Item 1: validation before M3

- [x] MIT license and requested copyright in LICENSE, package metadata and runtime image.
- [x] Rust 1.88 formatting, strict Clippy and all 16 tests.
- [x] linux/amd64 release image build, mock Compose validation and hardened runtime checks on local Docker Desktop.
- [x] RustSec audit with zero vulnerabilities/warnings after removing an unmaintained wrapper; CI gate added.
- [x] Image vulnerability scan completed and findings recorded in security-validation.md.
- [x] Replace Debian slim with signed, digest-pinned distroless and repeat checks: no high/critical findings, no exclusions. Track remaining medium/low findings.
- [x] Create fixtures/truenas-25.04.2.6/ and document captured-fixture intake.
- [ ] Supply genuine sanitized 25.04.2.6 payloads with provenance, then validate offline.

## Delivery status

M1/M2 functional work and all fifteen review findings now have recorded evidence. TN-02 is resolved for the captured pool.query scope; M3-A preflight and M3-B native/container system evidence are complete; M3-C opt-in connection gating/system collection is implemented and verified offline; M3-D realtime implementation is verified offline; native/container realtime preflight now passes, and four captured realtime notifications replay offline; numerical and integrated runtime acceptance remain open. See validation.md for this milestone and security-validation.md for the preceding security refresh.

## MEMORY ALPHA proxy integration and multiple targets

- [x] Update endpoint examples to wss://memory-alpha.home.lab/api/current.
- [x] Export the verified public Caddy root outside Git; native and container WSS upgrade pass with verification enabled. Container negative trust test rejects the untrusted CA.
- [x] Add a transport-only diagnostic that never loads/sends API credentials.
- [x] Add bounded multi-target configuration, independent workers/caches, stable target labels, combined metadata and partial-outage readiness semantics.
- [x] Validate multi-target credentials/CA isolation, identical pool names, independent expiration and log redaction against offline TLS mock servers.
- [x] Dedicated prometheus_exporter account and key provisioning by the operator.
- [x] Authenticated login and successful read-only pool.query with operator-confirmed 25.04.2.6 provenance. Minimum privilege and write denial remain untested.
- [x] Repeat authenticated pool checks inside Docker under UID 10001 with a restricted read-only secret volume.

Authenticated pool compatibility is established. M3 backend/system/realtime live validation needs its own authorization. No administrator credential was substituted and no infrastructure was modified. See memory-alpha-integration.md.

Offline operational continuation passed on the Mac and its local linux/amd64 Docker runtime. See the final measurements in validation.md and the refreshed signature/security evidence in security-validation.md. The subsequent TN-02 capture workflow is complete as recorded below.

## TN-02 authenticated compatibility milestone

- [x] Operator provisioned the dedicated account/key and confirmed Readonly Admin in the UI.
- [x] Explicitly authorized native and local Docker TLS/auth.login_ex/pool.query checks passed.
- [x] UID/GID 10001 read the restricted secret from a read-only temporary Docker volume; volume removed afterward.
- [x] Complete sanitized response structures saved with provenance; types, nulls and absent fields preserved.
- [x] Offline TLS replay correlates the captured envelope through Client, validates collector values, encodes metrics and verifies readiness.

The current gate for starting M3 is satisfied for pools. Broader live calls require additional authorization. At TN-02, version provenance was operator-confirmed only; M3-B below adds an API-reported version observation. Minimum account privileges, realtime compatibility and production deployment remain unverified.

## M3-A — explicit system preflight

- [x] Resolve system.info registration, role declaration and exact field/units mapping against pinned official 25.04.2.6 source.
- [x] Implement typed version/hostname/uptime/memory/load validation and an explicit, bounded check-system command.
- [x] Keep ordinary pool polling, metrics and readiness unchanged; test the new path only with synthetic fixtures and loopback TLS.
- [x] Obtain separate operator authorization and capture sanitized native/container system responses (M3-B); replay through the production preflight offline.
- [x] Integrate opt-in per-connection runtime version gating and independent required system snapshot/TTL/readiness (M3-C).
- [x] Implement source-verified opt-in realtime CPU/memory/network/ARC groups with independent freshness (M3-D), verified offline.
- [x] Complete authorized native/container preflight and sanitized realtime capture/replay for the observed up-link baseline.

The detailed current source mapping, command behavior, acceptance limits and next slices are in system-contract.md. This milestone does not establish live system.info support or complete M3.

## M3-B — observed system compatibility

The actual check-system command passed from macOS and local Docker Desktop under UID/GID 10001:10001 using the dedicated read-only credential and verified TLS. Separate memory-sanitized full system.info envelopes report version 25.04.2.6 and pass offline preflight replay with independently reviewed memory/uptime/load assertions. All live methods in this milestone were auth.login_ex and system.info. No decoder change was needed. See fixture provenance and memory-alpha-integration.md.

At completion of M3-B, the next slice was automatic version gating and scheduled system snapshots. M3-C below implements that slice behind an explicit opt-in flag. Realtime authorization remains separate.

## M3-C — opt-in system collector

TRUENAS_SYSTEM_ENABLED defaults false. Enabled targets gate every connection/reconnect before request admission, poll system and pools independently, and require both fresh snapshots plus compatibility for readiness. System info/uptime/physical total memory/load metrics and per-group operational history are implemented. Version rejection immediately masks measurements while keeping historical timestamps; ordinary per-group errors retain bounded cache grace. A scheduler completion cannot reopen a rejected gate. Mock mode remains network-free.

Focused unit/TLS/process tests, full Rust/MSRV checks and the enhanced container mock/hardening validator pass; see validation.md for commands, the transient existing test failure and scope limits. No live scheduled polling, new appliance method, infrastructure edit or deployment occurred. Detailed design decisions are in m3-c-implementation.md. Next: M3-D realtime contracts and per-group event handling, then callable scan metrics after schema validation. Previously recorded live acceptance covers individual pool/system calls, not the new integrated daemon mode.

## M3-D — realtime implementation, live acceptance pending

The source-pinned separate connection, owned subscribe/unsubscribe lifecycle, bounded latest snapshots, CPU/memory/network/ARC mappings, independent event-group TTLs and required-group readiness are implemented behind TRUENAS_REALTIME_ENABLED=false. The flag requires system collection; synthetic mode never opens sockets. check-realtime is ready for a separately authorized one-shot live preflight.

Offline validation covers conversions, up/down/missing fields, partial-group failures, limits and duplicate keys, exact independent expiry, event receipt time, foreign collection filtering, pre-ack replacement, permission/remote-unsubscribe recovery, silence despite pongs, bounded cancellation cleanup, real daemon dual-socket metrics/redaction/SIGTERM and container mock operation. Source gaps, account role permission, actual frames, UI/Graphite equivalence and full enabled-mode resource budgets remain explicit acceptance limits. See realtime-contract.md and validation.md. No live realtime calls were made. Next after authorized evidence: scan metrics, remaining operational acceptance and expanded monitoring/dashboard handoff.

## M3-D live preflight evidence

The operator separately authorized check-realtime from macOS and local Docker Desktop against the configured WSS endpoint with the existing dedicated credential. Both completed successfully: verified TLS/authentication/system compatibility, fresh selected CPU/memory/network/ARC groups and confirmed owned unsubscribe. Temporary Docker secrets were removed; no raw payload or captured realtime fixture was saved. See memory-alpha-integration.md.

This closes the one-shot live permission/schema preflight gate. The later capture baseline below completes reviewed normal-frame capture/replay. Remaining realtime acceptance is observed down/missing cases where naturally available, numerical UI/Graphite equivalence and measured integrated runtime/resource/outage behavior. Normal feature defaults remain off and no production configuration was changed.

## Captured realtime baseline and replay

Under the continued read-only event authorization, two notifications each were captured from the Mac and UID-10001 Docker runtime, with exact API-reported version 25.04.2.6 and owned cleanup acknowledged before writing. Interfaces are aliased consistently within each capture; full event structure, numeric types/values and existing omissions are preserved. No production code changed. All four frames replay unchanged over loopback TLS through the production worker and metric encoder with independently reviewed expected values.

All interfaces observed were up; down/missing/backend-failure cases remain explicitly synthetic unless naturally observed later. Selected ARC hit percentages are zero in all captured frames, and upstream defaults prevent inferring actual cache efficiency from those zeros alone. Next acceptance work is independent numerical/source-chart verification and integrated resource/outage testing. No failure state should be induced on the appliance to obtain an optional fixture. See captured-fixture provenance and validation.md.

## Numerical source review — November 14, 2026

The offline source-to-wire review and synthetic upstream-function experiments are complete; see realtime-numerical-review.md. Exporter conversions agree with the pinned adapter. Upstream missing-data defaults and ARC cumulative-percentage/chart-algorithm semantics prevent closing independent numerical acceptance. No appliance access or production behavior change occurred. Paired operator-supplied Reporting evidence or separately authorized bounded source-chart inspection remains necessary. Controlled local integrated resource/outage testing can proceed independently.


## Integrated finite operational baseline — November 14, 2026

- [x] Exercise opt-in system/realtime collection together with maximum target/pool/CPU/interface cardinalities on native and network-disabled Docker loopback sources.
- [x] Verify realtime silence does not refresh expired groups or invalidate fresh polling; verify full outage expiry/recovery and owned shutdown requests under a stalled metrics reader.
- [x] Record scoped native child and Docker child/cgroup CPU/RSS observations; preserve ordinary-suite opt-in behavior for sustained tests.
- [ ] Profile longer steady-state and repeated recovery to establish memory stability/headroom; finite passes are not a universal budget guarantee.
- [ ] Complete independent paired live numerical/source-chart verification and remaining role/CI acceptance before production readiness.

See integrated-operational-validation.md and validation.md for results. Production code/defaults are unchanged. The next resource investigation should distinguish allocator retention and fixture/client overhead from exporter growth before selecting a correction or changing limits. Scan callable contracts remain a separate subsequent milestone.


## Phased memory-profile continuation — November 14, 2026

- [x] Run a longer warmed-baseline/load/cooldown comparison and three outage/recovery cycles on native macOS and capped network-disabled Docker, with per-phase child CPU/RSS and durable synthetic evidence.
- [x] Record the scrape-associated native RSS peak, cooldown reduction and nearly flat quiet-recovery minima without inferring allocation ownership or indefinite leak freedom.
- [x] Complete finite separate exporter/fixture cgroup accounting; the later isolated baseline records 63.30 MiB maximum sampled exporter charge under 128 MiB. True kernel high-water/long-term production budget confidence remains outside this finite result.

See operational-memory-profile.md and docs/evidence/operational-memory-profile-2026-11-14.json. Independent live numerical/role acceptance and the historical intermittent process-fixture reliability issue remain open. Callable SCRUB/RESILVER schema work remains the next feature milestone; no production collector change is justified by this profile alone.


## Separate-cgroup resource baseline — November 14, 2026

The phased workload passed with the production exporter in its own 128 MiB/half-core cgroup and the fixture in a separate 256 MiB/one-core cgroup. External read-only Docker stats recorded a 63.30 MiB highest sampled exporter charge (64.70 MiB sampled margin) and 53.42 MiB fixture charge. All recovery/shutdown assertions passed; exporter exit was zero and OOMKilled=false. No production code, limits or infrastructure changed.

This closes the finite attribution milestone, not a universal production budget. The exporter kernel high-water mark was unavailable; memory-event history, sampling gaps, payload variation and longer soaks remain explicit limits. See isolated-cgroup-validation.md and its durable evidence. Next feature work is callable SCRUB/RESILVER contract review using pinned source and captured evidence; live numerical/role/CI/deployment gates remain separate.
