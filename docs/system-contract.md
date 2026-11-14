# M3-A system preflight contract

Status: implemented, tested offline and verified by operator-authorized native/container system.info checks; full sanitized responses are captured and replayed offline. This is a manual preflight, not a scheduled collector or daemon-wide runtime gate. Normal pool polling, metrics and readiness retain the M2 contract.

## Exact source evidence

Inspected official TrueNAS middleware tag TS-25.04.2.6, resolved commit `244b717370fe35fb9eefc3096acfbc25f12b57b6`:

- [system.info implementation and legacy schema](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/plugins/system/info.py): accepts no positional arguments, declares READONLY_ADMIN, returns version, hostname, uptime_seconds, physmem and loadavg among other fields. It does not carry the private decorator. Internal hostname/time/memory helpers are private and are not called by this exporter.
- [API construction](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/main.py): _create_apis maps current to the current API; _get_methods and _create_api register service methods, including methods using older schema declarations. [Method.call](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/api/base/server/method.py) authorizes the method and dumps its return value through middleware.
- [Documentation generation](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/api/base/server/doc.py) excludes methods without new_style_accepts. This explains why the published 25.04.2 method catalog omits system.info; catalog absence alone does not establish method absence. This is source-derived callable evidence, not a successful appliance call.
- [Version source](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/plugins/system/product.py): system.info uses system.version_short, which returns the manifest version. system.version returns a branded full name and is not called by this implementation. [sw_info](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/utils/__init__.py) constructs those two forms separately.
- [WebUI alternative](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/plugins/webui/main_dashboard.py) is explicitly intended for the WebUI team and is not selected as the exporter source.

No fallback API calls, method discovery, webUI methods, local appliance files or shell commands are used. The READONLY_ADMIN role declaration is source evidence; successful dedicated-account read authorization was subsequently observed in M3-B; minimum privilege and denial of writes remain untested.

## Selected field contract

| Field | Representation and meaning | Validation |
| --- | --- | --- |
| version | Manifest short version string | At most 64 ASCII digits/dots, three or four u32 components; exact family 25.04.2 or 25.04.2.N |
| hostname | OS socket.gethostname() value | Required, 1..253 UTF-8 bytes, no control or whitespace characters |
| uptime_seconds | CLOCK_MONOTONIC_RAW seconds since boot | Required finite nonnegative JSON number; preserve fractional seconds |
| physmem | /proc/meminfo MemTotal multiplied by 1024 in the server | Optional positive u64 bytes; missing/null remain None; never multiply again |
| loadavg | os.getloadavg(), in 1/5/15-minute order | Optional array of exactly three finite nonnegative numbers; load is not a CPU percentage |

The implementation's memory helper can return null when it cannot read MemTotal, despite the older declared required integer schema. Missing/null optional memory/load values therefore remain absent. This is a deliberate tolerant measurement policy, not evidence of a captured missing/null system response. Required identity/version/uptime cannot be absent or null. Strings, booleans and fractional/negative memory sizes are not coerced. Unknown fields, including license, system_serial and hardware identity, are ignored by the typed result and never displayed.

Syntactically numeric releases outside 25.04.2.x return unsupported. Invalid formats, branded versions, development suffixes and invalid selected fields return schema. Acceptance of the family does not claim captured evidence for every patch; 25.04.2.6 remains the source/capture baseline.

## Explicit command and bounds

`truenas-exporter check-system` loads normal target configuration and requires exactly one non-mock target. It opens verified WSS, performs typed auth.login_ex, sends one system.info request with params=[], validates its selected fields and compatibility, closes the connection, and exits. It sends no pool query, subscription or retry. It does not bind an HTTP listener, alter cached state or change readiness.

One API-timeout budget covers connect, authentication, request send, response wait and close. Existing 1 MiB frame/message bounds and TLS/hostname verification apply. Ping frames are answered; unrelated valid notifications/IDs do not satisfy or extend the deadline. Failure output contains bounded reason categories; success output contains only a fixed confirmation. Hostname, version details, uptime, credentials, raw frames and remote error messages are not printed. Diagnostic mode does not initialize dependency tracing, including when RUST_LOG=trace.

A live invocation requires explicit authorization for auth.login_ex and system.info on the selected endpoint, plus the dedicated read-only account/key. Prior pool.query authorization does not cover system.info. The operator separately approved the M3-B checks; the recorded runs completed under that approval. This command is never part of CI against a real appliance.

## Acceptance and next slices

- M3-A: source mappings, closed SystemInfo method, typed selected-field parser, explicit bounded check-system and offline CLI/TLS tests. Implemented.
- M3-B: separately authorized native and UID-10001 Docker system.info checks passed. Full responses were sanitized in memory and replayed offline with capture/version provenance; see fixtures/truenas-25.04.2.6/README.md. API-reported version is 25.04.2.6. Completed for this observed response scope.
- M3-C: integrate per-connection compatibility checks with scheduled system polling, finite system TTL and independent operational metrics. Define required/unsupported group readiness before enabling the collector; test version changes on reconnect and failure/expiry isolation from pools. No system metric or automatic runtime gate is delivered by M3-A.
- M3-D: exact reporting.realtime subscription/event contract, bounded latest-value delivery and CPU/memory/network/ARC group validation with independent freshness. Missing/down-link rates remain absent. Separately authorize live subscribe/unsubscribe only after the concrete path is ready.

Synthetic fixtures remain in application/tests/fixtures/. Captured fixtures remain in the root fixtures/truenas-25.04.2.6/ directory. The new system fixture is explicitly synthetic and contains fictional identity/serial markers for output-redaction tests.
