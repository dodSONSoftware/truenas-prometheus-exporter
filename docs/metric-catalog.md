# Metric catalog (M1/M2)

Prometheus/OpenMetrics encoding includes HELP and TYPE. All unsupported/null measurements are absent. No NaN/Inf. Labels are limited to collector=pools, bounded reason categories, and validated pool identity/status values. Prometheus supplies job/instance. There is no implicit host label in M2.

| Metric | Type | Source / semantics |
| --- | --- | --- |
| truenas_api_connected | gauge | 1 only after successful authentication, 0 on disconnect |
| truenas_exporter_connection_errors_total{reason} | counter | Failed transport/authentication sessions, using the same bounded reasons |
| truenas_exporter_mock_mode | gauge | Explicit synthetic mode 0/1 |
| truenas_exporter_collection_success{collector} | gauge | Last attempt succeeded AND snapshot remains fresh |
| truenas_exporter_last_attempt_timestamp_seconds{collector} | gauge | Wall epoch at most recent attempt; absent before first attempt |
| truenas_exporter_last_success_timestamp_seconds{collector} | gauge | Wall epoch of latest valid snapshot; retained after expiry |
| truenas_exporter_snapshot_age_seconds{collector} | gauge | Monotonic age; absent before first valid snapshot |
| truenas_exporter_collection_duration_seconds{collector} | gauge | Monotonic duration of last attempt; absent before first attempt |
| truenas_exporter_collection_errors_total{collector,reason} | counter | Failed attempts, reasons unavailable/timeout/transport/tls/auth/expired/otp/redirect/permission/unsupported/api/protocol/schema/busy |
| truenas_pool_size_bytes{pool} | gauge | pool.query size, integer bytes >= 0 |
| truenas_pool_allocated_bytes{pool} | gauge | allocated, integer bytes >= 0 |
| truenas_pool_free_bytes{pool} | gauge | free, integer bytes >= 0 |
| truenas_pool_healthy{pool} | gauge | healthy boolean, independent of warning |
| truenas_pool_warning{pool} | gauge | warning boolean, FEAT_DISABLED is not device degradation |
| truenas_pool_info{pool,status,status_code} | gauge | 1; status allowlist ONLINE/DEGRADED/FAULTED/OFFLINE/UNAVAIL/REMOVED/SUSPENDED, otherwise UNKNOWN; status_code retains verified FEAT_DISABLED, other codes map to OTHER, absent codes to UNKNOWN |
| truenas_pool_fragmentation_percent{pool} | gauge | fragmentation numeric or numeric string, 0..100 |

Scrub/resilver decision: future metrics use truenas_pool_scan_errors{pool,function}, truenas_pool_scan_progress_ratio{pool,function}, and truenas_pool_scan_state{pool,function,state}; bounded function SCRUB/RESILVER and state SCANNING/FINISHED/CANCELED. Errors is a gauge, not a lifetime counter. No scan means absence. Do not implement the ambiguous truenas_pool_scrub_errors name. Scan metrics are deferred to M3 until the complete callable response contract is verified; M2 ignores scan content.

M3 mapping gate: document exact supported methods/fields for hostname, version, numeric uptime, physical total/available memory and load. Reporting CPU percentage converts once to ratio; link Mbps converts once to bits/s; receive/transmit byte rates remain gauges. Missing down-link rates do not become zero by default. Event documentation does not prove callable method shape. Realtime failure isolation is per CPU/memory/network group; validating one group must not refresh another group's timestamp. Each group needs its own TTL and required/unsupported readiness decision before implementation.

Tests cover exposition metadata, label escaping, units, expiry boundaries, absent optional fields, invalid ranges, empty results and simultaneous healthy/warning. Pools are capped at 256 and names/codes bounded in length. No GUIDs, paths, messages or timestamps as labels.

New source status codes require a reviewed mapping before becoming label values. Raw strings are validated, then metadata is normalized to the finite label vocabulary. Label escaping is explicit because the selected client string encoder writes raw label values.

## Multi-target labels

File-configured targets and named single targets add a stable `target` label to every row in this catalog. It comes only from a validated configuration ID, never from credentials, URLs or API data. The existing collector/pool/reason/status labels remain. Queries intended for one NAS should select `{target="memory-alpha"}` to avoid mixing servers with identical pool/interface names. Legacy unnamed single-target mode preserves old labels.

A failed target's success/error/age series retain its own target label, and its expired measurements disappear independently. There is no duplicate HELP/TYPE metadata when multiple targets expose the same metric family.
