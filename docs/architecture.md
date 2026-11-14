# Architecture and failure contract

One executable and Cargo package. Config validates local inputs before starting. A connection task owns the WebSocket and bounded request map. The pool scheduler performs asynchronous reads. HTTP shares immutable pool data under a short lock and encodes outside the lock; it never calls TrueNAS.

## Deterministic state

Default mode requires pools only. M3-C adds an explicit system-enabled mode requiring both pool and system snapshots, with separate state/timestamps/errors and shared configured poll/TTL defaults. Realtime is disabled by default. M3-D adds four independent required event groups only when explicitly enabled.

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

The explicit check-system command retains its one-shot behavior and does not refresh daemon state. M3-B added live method/capture evidence, and M3-C integrates the same source-pinned decoder into optional system collection; see system-contract.md.

## M3-C connection gate and independent snapshots

When enabled, the supervisor validates system.info after authentication and before exposing connected=1 or admitting requests, on every connection. A successful gate seeds a system snapshot; system and pool scheduler loops then poll independently on the same bounded client. The existing initial unavailable tick behavior can still delay pool readiness by up to a poll interval.

Typed Collection<T> state shares attempt/success/error/TTL metric semantics without sharing timestamps or payloads. Snapshots hold Arc data; a scrape clones the small state and Arc references rather than cloning pool vectors. Optional system fields remain absent. A compatibility flag belongs to each target and can be reopened only by its connection gate, so late scheduler completions cannot bypass rejection.

Replies select a result decoder from the pending request's closed method variant. Pool results retain the 256-entry preallocation guard; system results must be objects of at most 64 KiB. Unknown IDs are validated as envelopes but do not expand result trees. Existing frame/message/request/queue limits remain. Scheduled invalid-version or unreadable/unsupported system results reject compatibility and terminate the session; ordinary selected-field validation or permission errors stay collector failures. Disconnects retain bounded cache grace; a failed gate masks both target measurement groups immediately while preserving operational history. HTTP never contacts the NAS.

## M3-D isolated realtime stream

Enabled targets have a second independently authenticated/version-checked socket for realtime. ApiConfig/credentials are cloned once per enabled target (bounded existing file limits; no Debug/log output); polling retains its own request map and scheduler. Subscription failures cannot block its writer or queue. A known incompatible realtime version revokes shared compatibility and wakes polling to reconnect; successful event delivery never reopens the polling gate.

The realtime worker owns exactly one acknowledged subscription ID and one optional pre-ack typed frame. A message publishes independently into four typed latest snapshots under short group locks. No per-event tasks or unbounded application queue are created. All four event groups use a separate configured realtime TTL with independent monotonic timestamps; neither pongs nor a sibling's valid fields refresh a failed group. HTTP remains cache-only, and combined readiness includes all enabled required groups.

Stream silence, remote unsubscribe and transport failures retry with existing capped jittered backoff. Shutdown attempts only the owned unsubscribe and has a one-second cleanup budget, concurrent with the existing HTTP drain. Synthetic mode uses no sockets and is explicitly marked. Offline protocol/process/container checks cover this implementation; live realtime/schema and full two-sockets-per-target resource measurements remain separate gates.
