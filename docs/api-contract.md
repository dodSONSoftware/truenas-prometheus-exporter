# API protocol contract (M2 baseline and M3-A preflight)

Established before M2 implementation. Transport is JSON-RPC 2.0 on a persistent verified WSS connection; no legacy /websocket protocol. Normal mode rejects ws://, URL credentials, queries and fragments. A private CA extends Mozilla roots. Both connection entry points revalidate the same WSS URL constraints before opening a socket, including for directly constructed Rust configurations. Loopback protocol fixtures use synthetic certificates and explicit private trust roots.

## Explicit wire allowlist

| Operation | Parameters | Implemented |
| --- | --- | --- |
| auth.login_ex | One object: mechanism=API_KEY_PLAIN, username, api_key, login_options.user_info=false | M2 |
| pool.query | Empty positional array; full list result required | M2 |
| core.subscribe | Exactly reporting.realtime:{"interval":N}, integer N 2..300 | M3-D opt-in realtime socket/check-realtime |
| core.unsubscribe | Only the ID returned by this connection’s successful subscribe ACK | M3-D bounded cancellation/check cleanup |
| system.info | Empty positional array; selected version/identity/uptime/memory/load validated | Explicit check-system and opt-in M3-C connection gate/scheduler |

Authentication is session establishment, not a NAS administrative write. Only typed method selection is exposed; no arbitrary method string or recursive discovery. Authentication never follows REDIRECT URLs. AUTH_ERR, EXPIRED, OTP_REQUIRED and REDIRECT are bounded error categories. JSON-RPC success alone does not authenticate a session.

Requests have unique string IDs. Responses may arrive out of order. Unknown/late IDs are ignored, malformed JSON/envelopes terminate the connection and fail pending calls. JSON-RPC error message/data and full frames are never logged. Method errors retain only a bounded category (permission, unsupported, api); schema errors are distinct. The decoder requires jsonrpc=2.0 and exactly one of result/error for responses. Errors require an integer code and string message. Notification params may be omitted, an object or an array; scalar and null params terminate the session as protocol errors. Error text is validated without being logged.

Limits: 32 queued commands, 32 in-flight calls, 1 MiB frame/message size. Admission, writer queue, socket write and response waiting share the absolute request deadline. Expired queued queries are skipped and expired callers receive timeout. Expiry during a socket write ends the session because delivery may be partial. Closed receivers and timed-out map entries are reaped. No automatic replay of individual requests; polling retries safe reads. A failed connection fails its pending requests. Authentication precedes all pool calls.

Connect/auth/write/read waits are bounded by the API deadline. Authenticated idle sockets receive WebSocket pings; missing inbound traffic closes the connection after two deadlines. Ping frames are answered. Reconnect uses exponential capped backoff (1 to 60 seconds) with jitter and resets only after at least 120 seconds of continuous authenticated uptime. Connection and authentication waits do not contribute to stability; each retry starts without an authenticated timestamp. Cancellation interrupts connect, authentication, polling and backoff. Persistent authentication failures keep HTTP alive and retry with backoff. Logs contain only bounded categories; TLS errors instruct the operator to check trust, validity and DNS/IP certificate identity.

Future subscription demultiplexing must parse method=collection_update with params.collection, params.msg and params.fields, and method=notify_unsubscribed. Slow consumers need bounded latest-value delivery. No subscription freshness is claimed by M2. Runtime version discovery and full realtime envelopes remain explicit M3 gates.

## Integration transport and scope

The approved MEMORY ALPHA URL is now wss://memory-alpha.home.lab/api/current, through Caddy. The optional CA file extends trusted roots without disabling name checks. check-transport performs only verified WSS upgrade/close and does not send login or JSON-RPC method frames. Authenticated appliance tests remain gated on operator-provisioned read-only credentials. See memory-alpha-integration.md for actual outcomes.

Multi-target mode gives each target its own client instance; the closed method allowlist is unchanged. No user-supplied method names, arbitrary API discovery, mutating operations or runtime HTTP target routing are exposed.

## M3-A system preflight

The closed SystemInfo variant was introduced by check-system, through the shared verified socket/authentication helpers. It validates the selected source fields and 25.04.2.x compatibility under one API-timeout budget, with no retries or subscriptions. M3-C routes results by pending method, retaining the bounded pool decoder and adding a 64 KiB object-only system result decoder. See system-contract.md for pinned source evidence, exact field rules, catalog omissions and the opt-in automatic runtime gate. M3-B subsequently verified separately authorized native and UID-10001 Docker system.info calls and captured full sanitized responses; see memory-alpha-integration.md. No subscription or additional method is implied by those checks.

M3-C enables scheduled system.info only with TRUENAS_SYSTEM_ENABLED=true, default false. Every enabled connection checks the version/selected core fields before setting connected=1 or admitting requests; reconnects repeat the gate. Unsupported or unvalidated versions fail closed, with immediate measurement/readiness suppression for the target. System methods return typed data to the independent scheduler; optional field/permission failures retain per-group bounded freshness. M3-D separately adds narrowly scoped lifecycle helpers on an isolated realtime socket; they are not generic polling methods. See configuration.md for the exact readiness/grace and gate-failure policy.

## M3-D lifecycle and event demultiplexing

The separate realtime connection authenticates and validates system.info before sending one core.subscribe request with params=[the exact configured dynamic collection name]. The acknowledged string ID is bounded to 128 bytes and stored privately on that session. Only that ID can be sent to core.unsubscribe; the expected result is null. No externally supplied method/event/ID routing is exposed. Reconnect creates a fresh subscription and never reuses an old socket's ID.

collection_update is accepted only for the exact full collection name and msg=added. Its CPU/memory/network/ARC groups are decoded independently. notify_unsubscribed for that collection ends the stream and triggers bounded reconnect/resubscribe, keeping only bounded cache grace. Other collections/notifications never refresh freshness. One pre-ack typed event can be retained; replacement preserves the chosen event's original receipt time. No event queue or task list grows with source traffic.

Application limits: text <=256 KiB, fields <=128 KiB, response ACK <=4 KiB, aggregate plus 256 cores, 128 interfaces; shared TLS wire limit remains 1 MiB. No matching events for realtime TTL triggers retry even with pongs. Cancellation interrupts connect/read/write/backoff; owned unsubscribe/ACK/close cleanup shares at most one second. Failure output/logs are bounded categories, never raw errors or subscription IDs. See realtime-contract.md for source links and live acceptance limits.
