# Current project status

Release 0.8.0, Steel Ferret, follows documentation checkpoint 3ac2903. Root VERSION is the release authority.

## Implemented and evidenced

| Capability | Current disposition |
| --- | --- |
| Original review TN-01 through TN-15 | All fifteen have recorded fixes/evidence; TN-02 captured pool scope completed in 4d7a29f |
| Pool monitoring | Capacity, allocated/free, healthy/warning, bounded status/status-code, fragmentation and scan errors/progress/state; pool-only mode is default |
| System monitoring | Opt-in system.info connection/reconnect version gate and scheduled identity/version/uptime/physical-total/load collection |
| Realtime monitoring | Opt-in separate authenticated socket; CPU, available physical memory, network state/speed/rates and ARC size/optional source-reported hit ratios; independent freshness |
| Multiple targets | 1..16 targets; isolated credentials/transports/snapshots/errors; all required groups/targets govern readiness |
| Appliance evidence | Authorized native and UID-10001 Docker auth/pool/system/realtime checks; observed version 25.04.2.6; sanitized full pool/system envelopes and four realtime frames replay offline |
| Finite operational evidence | Dual-socket integration, silence despite pongs, full outage/recovery, repeated recovery and stalled-reader shutdown passed against synthetic loopback services |
| Resource attribution | Separate-cgroup profile passed: maximum sampled exporter charge 63.30 MiB under 128 MiB; fixture 53.42 MiB separately. Exporter true kernel peak/event history unavailable |
| Ordinary checks at baseline | 83 regular Rust tests, 16 Python tests, formatting and strict Clippy passed; Rust 1.88 checks/profile builds passed within recorded scopes |
| Sustained checks | Four explicitly ignored scenarios; isolated_cgroup_memory_profile requires its controller, not bare cargo test --ignored |
| Packaging/security | Pinned non-root distroless runtime, license, shell-free probes and offline Compose/integration examples. Security scans are dated evidence, not current vulnerability guarantees |

System/realtime default false. Default polls are 60 seconds; API timeout 10 seconds; pool/system TTL three poll intervals; realtime interval 2 seconds and TTL three intervals. /metrics reads cached state only; /healthz is process liveness; /readyz requires every enabled required group and target to be fresh/compatible. Failed attempts never refresh a snapshot. Expired measurements disappear, and upstream-supplied zeros cannot recover lost availability provenance.

## Open work and gates

1. **Independent numerical verification.** Offline captured-value and pinned-source reverification is recorded in realtime-numerical-review.md. The exact proposed live windows are in numerical-live-comparison.md; reporting-api-comparison-plan.md verifies the public chart calls and separate local diagnostic scope. An authorized 45-event window on November 14 completed with cleanup; selected evidence is committed. A tested explicit public-chart diagnostic now exists. Its separately approved corrected window completed 13 calls and returned all 12 selected charts. Receipt-aligned residuals and public ARC zero percentages are recorded. Independent raw-counter/metadata numerical evidence remains absent. See public-chart-comparison-results.md. ARC utility/chart algorithm semantics and upstream zero defaults remain unresolved. Preserve faithful wire values; do not claim actual cache efficiency or introduce a correction without evidence.
2. **Combined live-daemon acceptance.** The operator-authorized local Docker daemon now serves scheduled pools/system plus realtime, with healthy HTTP/readiness and live Prometheus scraping. Formal bounded shutdown/recovery acceptance remains separate. Prepare a bounded read-only action and check authorization before expanding scope. No induced NAS fault is needed.
3. **Least privilege and deployment credentials.** Effective selected reads succeeded; Readonly Admin assignment is operator-reported. Minimum effective roles, denial of writes and eventual Linux secret/CA ownership/rotation are not independently established. No write probes or role changes are implicit.
4. **Intermittent process-test reliability.** Existing configuration/process tests have occasionally failed with ConnectionReset/early exit then passed in isolation/full reruns. Latest milestone passed; root cause remains unresolved. Investigate without weakening assertions or adding unexplained retries.
5. **Final candidate CI/security.** Hosted GitHub CI has not run. Execute exact final candidate Rust 1.88/stable, container/hardening, dependency/image gates and appropriate opt-in profiles; refresh security evidence when justified. Medium/low findings remain tracked in dated reports. Production-representative soaks can refine resource confidence; samples are not hard bounds.
6. **Monitoring artifacts and rollout.** A complete dashboard and local Compose override are implemented and running on the authorized Mac stack. Review Compose/scrape/target examples against the actual Linux stack and create reviewed alert artifacts with target/units/freshness semantics, parallel Graphite comparison and rollback. No live imports/configuration/deployment without authorization.
7. **Later scope.** P1 disks/inventory/reliable temperature/health, services and alerts follow P0 acceptance. P2 applications/tasks/VMs follow stable P0/P1. Arbitrary API discovery/recursive flattening is outside scope.

## Evidence and reading map

- [Implementation plan](implementation-plan.md): ordered work and completion criteria.
- [Restart handoff](handoffs/2026-11-14-handoff.md) and [pasteable prompt](handoffs/2026-11-14-restart-prompt.md).
- [Compatibility](compatibility.md), [API](api-contract.md), [configuration](configuration.md), [architecture](architecture.md), [metrics](metric-catalog.md): current contracts.
- [Captured fixture provenance](../fixtures/truenas-25.04.2.6/README.md), [appliance integration](memory-alpha-integration.md): actual live scope.
- [Numerical source review](realtime-numerical-review.md): source trace and unresolved ARC concern.
- [Integrated baseline](integrated-operational-validation.md), [phased profile](operational-memory-profile.md), [separate cgroups](isolated-cgroup-validation.md): finite synthetic evidence with measurement limits.
- [Validation ledger](validation.md) and [security ledger](security-validation.md): dated results; later records supersede earlier pending claims.
- [Historical implementation plan](history/implementation-plan-through-0.3.6.md): preserved sequential narrative.

External review/action plans live at /Users/randydodson/Documents/___code_review_action_plans/truenas_codereview-action-plan/. Their current index mirrors this status; findings.md remains the dated audit record. No production deployment, live dashboard edit, push or tag is recorded.
