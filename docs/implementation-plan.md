# Current implementation and acceptance plan

Updated November 14, 2026 for release 0.8.0. Use [project-status.md](project-status.md) for the current status and evidence map. The [earlier sequential plan](history/implementation-plan-through-0.3.6.md) is archived; its superseded checkboxes must not reopen completed work.

## Completed baseline

- [x] M1/M2 core Rust exporter, typed file-based credentials, verified WSS, bounded actor/deadlines, cached OpenMetrics and deterministic failure/expiry.
- [x] All original review findings TN-01..TN-15, including captured pool compatibility in 4d7a29f.
- [x] M3-A system preflight (79387b8), M3-B captured system evidence (24d7b8d), M3-C opt-in gate/collector (74c8575).
- [x] M3-D opt-in realtime implementation (4f0d215), native/Docker preflight (f89e193), four captured-frame replays (a9ce8f1).
- [x] Offline numerical source review (d69722f), with operational semantics still explicitly unresolved.
- [x] Finite integrated operational baseline (8e7d21f), longer phased profile (69ecd46), separate-cgroup attribution (069450d).
- [x] Local quality gates, pinned image builds and scoped Rust 1.88 checks recorded in validation.md. Latest baseline: 83 regular Rust tests, 16 Python tests; four sustained tests ignored by default.

- [x] Callable pool scan errors, progress and state implemented. Both FINISHED SCRUB captures replay. RESILVER and other unobserved variants have synthetic coverage. See [API contract](api-contract.md#callable-pool-scan-ts-250426).

## Acceptance work that can proceed alongside feature development

| Work | Required result / boundary |
| --- | --- |
| Independent numerical/chart comparison | Paired time/window/unit/identity evidence for CPU/memory/network; resolve ARC chart availability/algorithm before cache-efficiency interpretation. New live methods/private APIs/shell need concrete authorization |
| Combined appliance daemon | Bounded explicitly scoped run covering scheduled pools/system + realtime + HTTP/readiness/cleanup; selected one-shot evidence is already complete |
| Least privilege | Confirm actual minimum effective roles and eventual host credential/CA ownership/rotation; no automatic role edits or administrative write probes |
| Process-fixture reliability | Reproduce/capture the intermittent reset/early-exit cause and fix with a focused regression, not relaxed assertions |
| Release candidate checks | Rust 1.88/stable, ordinary tests, relevant opt-in profiles, hardened image/Compose, dependency/image security gates tied to exact commit/image; hosted CI still unrun |
| Production resource confidence | Longer representative soak/payload/scrape policy as needed. Separate-cgroup finite attribution is complete; exporter kernel peak/event history and universal bounds are not established |

## Delivery and later scope

- [x] Authorized Mac Docker Desktop stack with live exporter, local scrape job, complete Grafana dashboard and backup/rollback instructions.
- [ ] Final Linux Compose/scrape/target configuration, operator runbook and alert artifacts.
- [ ] Explicitly authorized Linux deployment and parallel Graphite acceptance; retain existing pipeline until an independent migration decision.
- [ ] P1 disks/reliable health-temperature, services and alert counts/severity after P0 contracts/acceptance.
- [ ] P2 applications/tasks/VMs after stable P0/P1; generic arbitrary discovery remains outside scope.

Automated validation stays synthetic/loopback. Past scoped appliance authorization is not blanket permission for new live methods or deployment. Preserve already authorized scope, prepare concrete bounded actions before asking for additional approval, and never read/print keys to reconstruct context.
