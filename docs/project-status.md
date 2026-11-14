# Current project status

Snapshot: November 14, 2026, documentation release 0.3.7, Tin Ferret. The last functional/tooling baseline is 069450d (0.3.6). Resolve the documentation checkpoint with git log; the root VERSION is release authority, while application/Cargo.toml intentionally remains 0.1.0. This page and implementation-plan.md supersede status claims in dated milestone records. Source contracts remain authoritative for implemented behavior.

## Implemented and evidenced

| Capability | Current disposition |
| --- | --- |
| Original review TN-01 through TN-15 | All fifteen have recorded fixes/evidence; TN-02 captured pool scope completed in 4d7a29f |
| Pool monitoring | Capacity, allocated/free, healthy/warning, bounded status/status-code and fragmentation; pool-only mode is default |
| System monitoring | Opt-in system.info connection/reconnect version gate and scheduled identity/version/uptime/physical-total/load collection |
| Realtime monitoring | Opt-in separate authenticated socket; CPU, available physical memory, network state/speed/rates and ARC size/optional source-reported hit ratios; independent freshness |
| Multiple targets | 1..16 targets; isolated credentials/transports/snapshots/errors; all required groups/targets govern readiness |
| Appliance evidence | Authorized native and UID-10001 Docker auth/pool/system/realtime checks; observed version 25.04.2.6; sanitized full pool/system envelopes and four realtime frames replay offline |
| Finite operational evidence | Dual-socket integration, silence despite pongs, full outage/recovery, repeated recovery and stalled-reader shutdown passed against synthetic loopback services |
| Resource attribution | Separate-cgroup profile passed: maximum sampled exporter charge 63.30 MiB under 128 MiB; fixture 53.42 MiB separately. Exporter true kernel peak/event history unavailable |
| Ordinary checks at baseline | 77 regular Rust tests, 11 Python tests, formatting and strict Clippy passed; Rust 1.88 checks/profile builds passed within recorded scopes |
| Sustained checks | Four explicitly ignored scenarios; isolated_cgroup_memory_profile requires its controller, not bare cargo test --ignored |
| Packaging/security | Pinned non-root distroless runtime, license, shell-free probes and offline Compose/integration examples. Security scans are dated evidence, not current vulnerability guarantees |

System/realtime default false. Default polls are 60 seconds; API timeout 10 seconds; pool/system TTL three poll intervals; realtime interval 2 seconds and TTL three intervals. /metrics reads cached state only; /healthz is process liveness; /readyz requires every enabled required group and target to be fresh/compatible. Failed attempts never refresh a snapshot. Expired measurements disappear, and upstream-supplied zeros cannot recover lost availability provenance.

## Open work and gates

1. **Next feature: general SCRUB/RESILVER metrics.** Verify the callable pool.query scan contract against pinned source and retained captures, decide missing/null/unknown/malformed behavior, then implement errors/progress/state with bounded labels and pool-cache freshness. No scan is absence, not zero errors. Both captures contain FINISHED SCRUB, errors=0, percentage=99.99960064888; do not force 100 or infer RESILVER evidence.
2. **Independent numerical verification.** Paired CPU/memory/network/chart observations remain absent. ARC utility/chart algorithm semantics and upstream zero defaults remain unresolved. Preserve faithful wire values; do not claim actual cache efficiency or introduce a correction without evidence.
3. **Combined live-daemon acceptance.** One-shot checks/captures do not establish scheduled pool/system plus realtime daemon acceptance on MEMORY ALPHA. Prepare a bounded read-only action and check authorization before expanding scope. No induced NAS fault is needed.
4. **Least privilege and deployment credentials.** Effective selected reads succeeded; Readonly Admin assignment is operator-reported. Minimum effective roles, denial of writes and eventual Linux secret/CA ownership/rotation are not independently established. No write probes or role changes are implicit.
5. **Intermittent process-test reliability.** Existing configuration/process tests have occasionally failed with ConnectionReset/early exit then passed in isolation/full reruns. Latest milestone passed; root cause remains unresolved. Investigate without weakening assertions or adding unexplained retries.
6. **Final candidate CI/security.** Hosted GitHub CI has not run. Execute exact final candidate Rust 1.88/stable, container/hardening, dependency/image gates and appropriate opt-in profiles; refresh security evidence when justified. Medium/low findings remain tracked in dated reports. Production-representative soaks can refine resource confidence; samples are not hard bounds.
7. **Monitoring artifacts and rollout.** Review existing Compose/scrape/target examples against the actual Linux stack; create reviewed dashboard/alert artifacts with target/units/freshness semantics, parallel Graphite comparison and rollback. No live imports/configuration/deployment without authorization.
8. **Later scope.** P1 disks/inventory/reliable temperature/health, services and alerts follow P0 acceptance. P2 applications/tasks/VMs follow stable P0/P1. Arbitrary API discovery/recursive flattening is outside scope.

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
