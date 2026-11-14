# Captured TrueNAS 25.04.2.6 fixtures

This directory contains **captured, sanitized** pool.query envelopes from a system the operator identified through its WebUI as TrueNAS SCALE 25.04.2.6. The exporter has not independently queried the backend version. These captures establish the observed pool response shape; they do not establish compatibility with every method or release.

Synthetic fixtures remain in `application/tests/fixtures/` and must never be relabeled as captured evidence. Do not contact MEMORY ALPHA or another live appliance to populate this directory without explicit operator authorization for that specific interaction.

## Intake requirements

- Preserve JSON types, optional/missing fields, nulls, units and response/event structure. Do not fill missing values or convert numeric strings during sanitization.
- Remove credentials, tokens, user details, serial numbers, GUIDs and confidential paths. Alias host, pool, interface and device identities consistently. Record fields removed or aliased so tests can distinguish sanitization from actual API omissions.
- Exclude authentication requests entirely. No raw key, password, cookie, session token or unredacted auth response belongs here.
- Use descriptive filenames, for example `pool.query.response.json` and later `reporting.realtime.notification.json`. These names are examples, not existing captures.
- Alongside each supplied fixture, record capture date, operator-confirmed appliance version and how it was established, method/event and parameters, whether the file is a full wire envelope or only a result, sanitization performed, and any uncertainty. Do not include identifying operator details.
- Review files locally for sensitive material before staging. Tests must consume saved files offline and require no credentials or NAS access.

M2 needs a full `pool.query` response. M3 additionally needs version/system and realtime frames, including normal, missing/null and down-link cases where available. Only assert behavior supported by the supplied evidence; do not invent captures for unavailable cases.

## November 13, 2026 capture provenance (November 14 UTC)

- Files: `pool.query.host.response.json` and `pool.query.container.response.json`.
- Source: explicitly operator-authorized read-only checks against `wss://memory-alpha.home.lab/api/current`, through the configured Caddy CA with hostname verification. Native macOS and Docker Desktop `desktop-linux` linux/amd64 under UID/GID 10001:10001 each returned one pool. The dedicated account was operator-provisioned as Readonly Admin; auth.login_ex returned typed SUCCESS before each pool.query.
- Method: `pool.query`, parameters `[]`. No version/system method, subscription or administrative method was sent. The operator reconfirmed 25.04.2.6 through the UI; that is the version provenance, not an API observation.
- Representation: full successful JSON-RPC 2.0 envelopes, with the string correlation ID replaced by `captured-pool-query`. Pool object keys, nested topology/scan structure, array lengths, booleans, nulls and original numeric types/values are preserved, except numeric identity fields named `id`, ending in `_id` or containing `guid`, which are replaced by numeric zero. No fields were removed or default values inserted. Serialization whitespace and key ordering were normalized.
- Strings: the top-level pool name is consistently aliased to `captured-pool-1`. String values for `status`, `status_code`, `fragmentation`, `function` and `state` are retained only when at most 64 characters of uppercase ASCII letters, digits or underscores. All other string values are replaced by `sanitized`, including GUIDs, paths, device/pool topology names, explanatory text and human-readable capacity strings. This deliberately limits string-value semantics beyond the current collector; repeated `sanitized` markers do not prove identity equality. Null path/device fields remain null.
- The original fragmentation string `"13"`, healthy=true, warning=true, status=ONLINE and status_code=FEAT_DISABLED are retained. Size is 24,000,277,250,048 bytes in both captures. Native allocated/free are 12,573,765,242,880 / 11,426,512,007,168 bytes; Docker allocated/free are 12,573,763,424,256 / 11,426,513,825,792 bytes. Each sums to size; sequential observations need not have equal usage.
- Raw responses existed only in probe memory. Authentication exchanges and credentials were never persisted. The key stayed outside Git and was not passed as an environment value or command argument. A restricted temporary Docker secret volume was removed after use.
- Offline regression: `captured_appliance_pool_envelopes_replay_offline` in `application/tests/api_client.rs` replays each saved envelope over loopback TLS through the production Client, replacing only its ID with the outstanding mock request ID. It asserts captured capacities and health/fragmentation, target/pool labels, successful HTTP metrics/readiness and absence of the deferred ambiguous scrub metric.
- Limitations: no alternate pool states, absent-field variant, permission-denial case, RESILVER or realtime payload was captured. Existing synthetic tests cover missing/null and error behavior; they remain synthetic. Nested scan/topology data is retained as shape evidence and is not yet a supported metric contract.
