# Architecture and failure contract

One executable and Cargo package. Config validates local inputs before starting. A connection task owns the WebSocket and bounded request map. The pool scheduler performs asynchronous reads. HTTP shares immutable pool data under a short lock and encodes outside the lock; it never calls TrueNAS.

## Deterministic state

Only pools is an implemented required collector in M1/M2. System/realtime are not registered and do not silently count as successful. M3 must explicitly define required groups when enabling them.

- Startup: connected=0, collection_success=0, no attempt/success timestamp, no measurement snapshot, readiness 503.
- Valid complete response: atomically replace the pool snapshot (including an empty list), record attempt/success/duration, success=1. Deleted pools disappear immediately on a valid replacement.
- Failed attempt: success=0, advance attempt/duration and a bounded reason counter; preserve the previous successful snapshot until its TTL. Never refresh last-success on failure.
- Disconnect: connected=0 immediately; pending calls fail. A completed collection failure marks success=0. Readiness depends on cached data freshness, so an outage can remain ready during the TTL grace period.
- Age >= TTL: omit all pool measurements, expose operational metrics and last-success/age, readiness 503. Expiration itself makes collection_success=0 even between attempts. No snapshot means no fabricated age or success timestamp.
- Recovery: the next valid snapshot restores readiness and success. A collection error does not reset process liveness.

TTL uses tokio monotonic time; exported epoch timestamps use wall time. HTTP computes expiry on each request, requiring no timer race to hide stale measurements. Default TTL is 180s, strictly greater than the 60s poll interval plus the 10s deadline. Polling skips missed ticks; never overlaps its own attempts.

Malformed required pool identity invalidates the complete collection. Missing/null optional measurements are absent. Present invalid measurements invalidate the collection, preserving the last valid snapshot until expiry. This avoids publishing a mixed-time partial snapshot. Healthy and warning remain independent. Empty pools is valid and ready. No scan never means zero errors.

/healthz always reports process liveness; /readyz reports 200 only while the required snapshot is fresh; /metrics exposes exporter state when admitted. One encoding job per router runs on the blocking pool. Overlapping requests during encoding receive HTTP 503 with "metrics encoder busy"; they do not queue encoding work. A disconnected request retains its admission slot until its encoder finishes. Health and readiness use independent routes. Completed response bodies and socket buffers have their own lifetimes. Responses contain no sensitive error detail. Liveness, never readiness, is the container restart check.

Mock mode performs no network access and periodically publishes one synthetic pool snapshot. It keeps truenas_api_connected=0 because there is no authenticated API connection; readiness can still be true. It is explicitly labeled via truenas_exporter_mock_mode and is not deployment evidence.

The scheduler starts immediately. If the API has not authenticated yet, its initial attempt fails as unavailable; collection resumes on the next poll tick, up to one poll interval after authentication.

## Target isolation

The executable now supports up to 16 explicitly configured targets. Each target owns its client/transport task, scheduler, collector snapshot and operational counters. A common cancellation token stops the workers; workers do not share credentials, CA additions, request correlation, backoff or freshness state. File-based configuration is validated completely before any connections start.

Metrics use one registry with gauge/counter families keyed by a configured target ID; descriptors and EOF are emitted once. Legacy unnamed single-target mode retains its original exposition. No arbitrary HTTP target parameter or user-selected scrape-time connection is provided.

All configured targets are required for readiness. A partial outage can therefore yield readyz=503 while other targets continue emitting validated, fresh measurements and metrics stays 200. Do not use readiness as an automatic restart policy.


## HTTP shutdown

SIGINT/SIGTERM cancels collector and connection workers and starts a two-second HTTP drain. Reading clients may finish within that interval. When the interval expires, main stops waiting for HTTP, joins the cancelled workers, and exits the Tokio runtime, dropping remaining connection tasks. A redacted warning identifies the drain deadline. The two-second interval bounds the HTTP drain wait; worker completion and any already running blocking encoder also contribute to total process exit time.

## M3-A preflight boundary

The explicit check-system command uses the shared validated WSS/authentication helpers for one source-pinned system.info call. Its typed decoder and compatibility check are independent of the pool supervisor/scheduler and never refresh pool state. No system collector is registered for metrics or readiness in this slice. Live system permission/schema evidence and per-group state integration remain later acceptance gates; see system-contract.md.
