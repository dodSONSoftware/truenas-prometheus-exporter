# Metric catalog (M1/M2 and opt-in M3-C)

Prometheus/OpenMetrics encoding includes HELP and TYPE. All unsupported/null measurements are absent. No NaN/Inf. Labels include collector=pools/system/cpu/memory/network/arc, bounded reason categories, validated pool identity/status values, and the explicitly enabled system metadata/finite load windows below. Prometheus supplies job/instance. There is no implicit host label in pool-only mode. Enabled system identity exposes a validated host/version tuple only on truenas_system_info, not on every series.

| Metric | Type | Source / semantics |
| --- | --- | --- |
| truenas_api_connected | gauge | 1 only after successful authentication, 0 on disconnect |
| truenas_exporter_connection_errors_total{reason} | counter | Polling-client transport/authentication/gate failures, using bounded reasons; realtime lifecycle failures appear in its group error rows |
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

## Enabled system metrics

These families are present only when TRUENAS_SYSTEM_ENABLED=true. The required system snapshot uses the configured monotonic TTL independently of pools. Expired/compatibility-rejected system measurements disappear; operational timestamps/errors remain. Mock samples are labeled by the existing exporter mock-mode gauge.

| Metric | Type | Source / semantics |
| --- | --- | --- |
| truenas_api_compatible | gauge | 1 when the connection gate is accepted and a validated system snapshot remains fresh; 0 before validation, after rejection/expiry, and always in mock mode. Omitted in pool-only mode. Does not imply a currently connected socket. |
| truenas_system_info{host,version} | gauge 1 | system.info hostname and manifest short version. At most one tuple per current fresh target snapshot; host 1..253 bytes with no whitespace/controls, version bounded to the supported numeric family. No serial/model/license identifiers. |
| truenas_system_uptime_seconds | gauge | system.info uptime_seconds, finite nonnegative seconds since boot, including the captured fractional component. Cached sample, not extrapolated time. |
| truenas_memory_total_bytes | gauge | system.info physmem, the server's kernel MemTotal multiplied by 1024. Integer byte value is not converted again. Missing/null is absent; not an installed-module capacity claim or available-memory estimate. |
| truenas_system_load_average{minutes} | gauge | system.info loadavg[0/1/2] over 1/5/15 minutes. Finite nonnegative load counts; not CPU percentage. Entire family absent for missing/null loadavg. Only minutes=1,5,15. |

System operational metrics reuse the catalog's existing attempt/success/duration/error/age families with collector=system. Success requires the last attempt to succeed, an unexpired snapshot and accepted compatibility; a failed latest attempt is zero even during cache/readiness grace. Both required groups and all configured targets must be fresh and compatible for readyz=200 in enabled mode. A system field/permission error does not reset a pool timestamp; a pool error does not reset system freshness. A version rejection immediately suppresses both measurement groups for the affected target. Other targets remain independent.

The source mapping and preserved units are recorded in system-contract.md and the captured fixture provenance. Captures are used in offline preflight and mock-daemon replay; M3-C did not start the new scheduled mode against MEMORY ALPHA.

## Enabled realtime metrics

These families require TRUENAS_REALTIME_ENABLED=true and TRUENAS_SYSTEM_ENABLED=true. They have independent CPU/memory/network/ARC freshness and the existing configured target label. All are gauges. The total-memory source remains system.info; realtime total is used for validating available <= total, not emitted as a duplicate family.

| Metric | Source and validation |
| --- | --- |
| truenas_realtime_subscribed | 1 after the separate realtime socket acknowledges its owned subscription, 0 on disconnect and always in mock mode. Fresh cached data may outlive the active socket only through TTL. |
| truenas_cpu_utilization_ratio{cpu} | cpu.cpu/cpuN.usage percent divided by 100 exactly once; finite 0..1. Aggregate cpu plus at most 256 canonical numbered cores. |
| truenas_memory_available_bytes | memory.physical_memory_available as supplied, integer bytes >=0 and <= positive physical_memory_total. This does not substitute ARC-free/available memory; deeper chart accounting and UI equivalence remain live acceptance work. |
| truenas_network_link_up{interface} | 1/0 only from LINK_STATE_UP/DOWN; unknown states invalidate the network group rather than produce zero. Names bounded to 64 ASCII interface characters, at most 128 entries. |
| truenas_network_link_speed_bits_per_second{interface} | Positive speed in Mbps multiplied by 1,000,000 once. Missing/null/zero and down-link speed are absent. |
| truenas_network_receive_bytes_per_second{interface} | received_bytes_rate, already bytes/s. Nonnegative finite gauge; missing/null or down-link rate absent. |
| truenas_network_transmit_bytes_per_second{interface} | sent_bytes_rate, same byte-rate rules. No second interval conversion and no counter semantics. |
| truenas_zfs_arc_size_bytes | memory.arc_size, nonnegative integer bytes. ARC validity/freshness is independent of physical-memory fields. |
| truenas_zfs_arc_data_hit_ratio | Optional zfs.demand_data_hit_percentage divided by 100 once, finite 0..1. |
| truenas_zfs_arc_metadata_hit_ratio | Optional zfs.demand_metadata_hit_percentage with the same conversion/range. |

Operational collector labels are cpu, memory, network and arc, using the existing attempt/success/duration/last-success/age/error families and bounded reason categories. Each group needs a valid snapshot to start; missing groups never fabricate zeros or timestamps. Optional ARC ratios do not block a valid ARC-size snapshot. Source-defined zero defaults can already erase upstream missing-data provenance; the exporter cannot recover that distinction from an explicit zero. No disk IDs, license/model/serial strings, raw event messages or subscription identifiers are exposed.

Wire/schema/unit evidence is pinned in realtime-contract.md. The checked-in realtime envelope is explicitly synthetic. Native/container preflight now verifies effective event-read permission and selected live schema validity. Captured frames, exact role inventory, numerical UI/Graphite comparison and integrated resource budgets remain unverified; a preflight pass is not full daemon acceptance.

Realtime processing duration measures selected-frame parsing, attributed to each affected group. A single lifecycle/backend failure can increment all four group error counters; summing them does not count unique connection failures. The existing connection-error counter remains owned by the polling client.
