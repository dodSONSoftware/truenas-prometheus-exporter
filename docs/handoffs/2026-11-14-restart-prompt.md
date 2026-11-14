# Pasteable TrueNAS exporter restart prompt

Paste the block below into a new conversation in this repository. It resumes implementation after reading the handoff; it does not authorize new live interactions or deployment.

```text
Continue the TrueNAS Rust exporter project at:
/Users/randydodson/Documents/_code/truenas_metrics_project

First read AGENTS.md, TrueNAS_Codex_Handoff.md, docs/project-status.md,
docs/implementation-plan.md and docs/handoffs/2026-11-14-handoff.md.
Then read the relevant API/configuration/architecture/metric contracts and
fixtures/truenas-25.04.2.6/README.md. Inspect Git status, current VERSION and
recent commits before editing; preserve unrelated user work.

Baseline: last functional/tooling commit 069450d, release 0.3.6; the handoff
checkpoint is documentation release 0.3.7, Tin Ferret. Root VERSION is the
release authority. All original TN-01..TN-15 findings have recorded fixes/
evidence. Pool and opt-in system/realtime collection, captured replay,
finite integrated/repeated recovery and separate-cgroup accounting are done.
Do not restart those milestones just because historical records say pending.

Begin the next implementation milestone: verify the callable pool.query
SCRUB/RESILVER contract against pinned TrueNAS middleware TS-25.04.2.6
(commit 244b717370fe35fb9eefc3096acfbc25f12b57b6) and saved captures. Then
implement only verified scan error/progress/state metrics with bounded labels,
missing/null/no-scan semantics, pool freshness and focused tests. Current code
ignores scan content. Both pool captures show FINISHED SCRUB with errors=0
and percentage=99.99960064888; do not force 100 or invent RESILVER evidence.
Do not trigger a scan. Public source review and synthetic tests suffice to start.

MEMORY ALPHA and production services remain off-limits outside specifically
recorded operator authorization. Past approvals covered bounded read-only
pool/system/realtime checks/captures from the Mac/local Docker, not arbitrary
chart/private APIs, shell, continuous deployment or administrative writes.
Prepare a concrete bounded action before requesting any genuinely additional
permission. Do not read/print keys, reuse disclosed chat material, broaden roles,
edit infrastructure/dashboards, push/tag/deploy, or create nested repositories.
Use local Docker Desktop context desktop-linux and generated loopback fixtures.
No subagents unless explicitly requested by the user or applicable instructions.

Keep limitations exact: ARC upstream zeros/chart semantics and paired numerical
verification remain unresolved; combined live-daemon/minimum-role acceptance,
intermittent process-test root cause, hosted CI and production rollout remain
open. Separate-cgroup sampled exporter peak was 63.30 MiB under 128 MiB; this
is not a kernel peak or universal resource guarantee. No allocator change is
justified by that evidence. Existing Graphite monitoring stays independent.

After each milestone, run appropriate repository checks and container build,
update current docs and the external action plans at
/Users/randydodson/Documents/___code_review_action_plans/truenas_codereview-action-plan/
and execute git-commit-message-instructions.md: inspect/stage intended changes,
increment root VERSION exactly once with proper semver, update README and
GIT_COMMIT_MESSAGE, and actually commit with git commit -F GIT_COMMIT_MESSAGE.
Do not change Cargo's package version or push/tag/deploy. Ordinary checks last
passed 77 Rust + 11 Python tests; four sustained cases are ignored by default,
and the isolated case must use its controller. Do not rerun long profiles
without a relevant change or unresolved question.

Proceed with offline scan-contract investigation after reading the handoff;
ask only if an actual blocking requirement or broader authorization is missing.
```
