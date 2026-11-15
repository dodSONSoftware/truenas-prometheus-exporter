# Live exporter operational acceptance — November 14, 2026

This record covers the operator-authorized execution of committed plan e2575bf. The controller is a one-shot operational test tool in this repository, not a separate monitoring application, NAS installation or permanent service. Its process exists only for the acceptance window and its detached restoration guard. The Rust exporter remains the deployed image 0.8.0; repository release 0.9.0 adds operator tooling and evidence, not a runtime deployment.

## Preparation and local rehearsal

scripts/live-operational-acceptance.py implements the plan's phased state machine, hard window and request limits, exact container/image/settings checks, cached HTTP and Prometheus observations, graceful stop, one planned start and detached restoration guard. Container mutations use the captured immutable container ID. The guard can start only that existing stopped container; it does not recreate or restart a running container. Core failure paths restore service and fail the test, rather than restarting the acceptance interval or hiding the failure.

Nine local tests exercise the same state machine, normal recovery, preflight refusal before mutations, interruption/restoration, stale and missing down-scrape evidence, broken stability, guard-assisted recovery rejection, other-service drift, subprocess deadlines/output caps, OpenMetrics validation, HTTP body/deadline bounds and a detached fake-Docker guard surviving parent exit. Tests use generated loopback HTTP and a fake Docker executable, never NAS credentials or the real Docker socket. The nine tests passed on native macOS and Python 3.13 Linux in a network-disabled, read-only container with an executable temporary fixture directory. The first Linux rehearsal mount was not executable, preventing the fake Docker script from running; the correctly configured fixture mount passed.

## Initial window: baseline refusal, no service mutation

The original authorized attempt began at 2026-11-15T03:49:59Z (November 14 in America/Chicago), passed deployment metadata checks and ended after approximately 0.49 seconds at its first dashboard query. The controller incorrectly classified a valid small Prometheus HTTP200 response as unavailable. It never armed a guard or issued stop/start. Read-only verification confirmed the exporter remained running/healthy with restart count 0 and surrounding container identities/start times unchanged.

A bounded diagnostic confirmed that this Linux Python HTTP implementation closes the underlying socket when the last Content-Length bytes are read. The controller attempted to set a timeout on that already closed socket before checking for completion, receiving EBADF and incorrectly returning an unavailable response. The fix terminates reading when the response is closed and also requires a complete declared body. A deterministic closed-on-final-read regression failed before the fix and passed afterward; native and Linux rehearsals then passed. This was a test-controller defect, not a NAS, exporter or Prometheus failure.

The plan prohibits automatic reruns. The operator explicitly authorized one corrected window with unchanged bounds before it was started in a fresh private task directory. The original baseline-failure record is retained alongside the corrected result evidence.

## Corrected window

**PASS.** The corrected window ran from 2026-11-15T03:56:53.674617Z through 04:01:29.669466Z, corresponding to November 14, 21:56:53–22:01:29 America/Chicago. Total duration was **275.995 seconds**, below the 480-second ceiling.

| Gate | Observed evidence |
| --- | --- |
| Baseline | 60 seconds of healthy/fresh cached HTTP and normal Prometheus scraping; seven expressions covering twelve NAS panels returned finite values |
| Shutdown | 2.435 seconds; Docker lifecycle kill signal 15/SIGTERM, stop and die exit 0; no SIGKILL/OOM |
| Stopped interval | Full 70 seconds; container remained stopped, direct HTTP unavailable, normal Prometheus produced a fresh up=0 sample after stop completion |
| Restart | Same container 23bf8673bd119bf411efc04b44d4f6cd602ba2298b9ecc92128ec42ae05363a3, same pinned image/mounts/settings, one planned start |
| Recovery | All six collectors recovered with last-success timestamps after new StartedAt and newer than baseline; normal Prometheus returned fresh up=1; recovery gate passed about 60.3 seconds after start completion |
| Stability | Full 80 seconds with no failed readiness/freshness observation; pool/system and realtime timestamps advanced again |
| Final | healthz200, readyz200, healthy/OOMfalse and successful fresh collection; all seven saved expressions recovered, covering twelve NAS panels; saved Grafana queries re-fetched unchanged |
| Guard | already-running; no restorative start or fallback was used, and the guard PID was confirmed gone afterward |
| Other services | prometheus/grafana/grafana-mcp/loki identities and start times unchanged; configuration hashes unchanged; 21 targets with the same three pre-existing down targets |
| Observation bounds | 56 observations, 168 direct HTTP requests and 62 Prometheus requests, below caps 288/112 |

The source controller SHA256 was c3cad1adf114341781294750507dfa81cc0115b5eb7f296e97733e2e1c2d49e2. Host/controller clock offset was approximately 0.00644 seconds with a 0.01906-second round trip. The compact [corrected evidence](evidence/2026-11-14-live-acceptance-corrected.json) retains phases, selected per-tick collector/measurement evidence, exact query bindings, flags, scrape sample times, guard disposition and baseline/final metadata. It records the full sanitized source report's hash and private Linux location. Missing startup measurements remain absent, not zero-filled. The [initial baseline failure](evidence/2026-11-14-live-acceptance-initial.json) remains a separate record. No NAS service, middleware, Netdata, network, credential, role, scan, Docker image or observability configuration change is part of this test. Normal exporter read-only API collection resumes on its unchanged schedule after restart.

## Scope of the conclusion

This PASS establishes the bounded deployed exporter shutdown/restart, scrape detection, fresh collection and saved-dashboard query recovery described above. It establishes query/data recovery, not screenshot rendering. It does not independently prove NAS unsubscribe acknowledgements/server-side session counts, raw numerical accuracy, ARC algorithm semantics, minimum roles/write denial, hosted CI, universal resource limits, long-duration soak behavior or induced NAS/network-fault recovery. The finite synthetic milestones remain separate evidence. No live image upgrade is included.


## Repository checks and retained state

Release 0.9.0 (Mercury Ferret) adds the reusable operator test harness; it is a MINOR tooling release. Cargo package version and production source are unchanged. scripts/check.sh passed formatting, strict all-target Clippy, 84 Rust tests and 27 Python tests. Four sustained cases remained ignored. The nine controller tests also passed in the network-disabled Linux rehearsal. The pinned linux/amd64 runtime image build passed in local context desktop-linux; that review image was not deployed. No push/tag or image publication was performed.

Private Linux task directories retain the non-secret controller/input/result audit files for the initial and corrected windows. No controller or guard remains running and no new permanent service was installed. The exporter is left running on its existing image. The original operational plan is preserved with an execution update and the external action plans are synchronized.
