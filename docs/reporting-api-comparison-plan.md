# Supported reporting comparison plan

Reviewed November 14, 2026 against middleware commit 244b717370fe35fb9eefc3096acfbc25f12b57b6 (TS-25.04.2.6). This is a proposed new observation window requiring operator approval. The previous 45-event approval covered its completed event-only window.

## Verified public surface and permissions

reporting.netdata_get_data(graphs, query) and reporting.netdata_graph(name, query) are public API methods declared with REPORTING_READ. Their cli_private flag hides CLI presentation and differs from private=True. The realtime event also requires REPORTING_READ. system.info declares READONLY_ADMIN in this revision. Authentication uses the existing dedicated identity and typed auth.login_ex. These are source declarations; effective minimum roles and appliance enforcement remain unverified. Stop on denied access and report it without modifying roles.

The production exporter allowlist remains unchanged. A reviewed one-shot comparison harness must explicitly allow only the methods and parameters below, enforce response/deadline budgets, test against loopback fixtures and support owned unsubscribe before any live execution. This document alone does not implement or authorize that harness.

## Exact public call sequence

Use one verified WSS connection from the Mac and existing dedicated file credential/CA. Total wall deadline 180 seconds, individual request timeout 10 seconds, no automatic retry/reconnect. One outstanding request at a time. Authentication and system version preflight precede the subscription. Stop if system.info does not report exactly 25.04.2.6. Credential content is loaded only through existing configuration code.

1. auth.login_ex with one object containing mechanism API_KEY_PLAIN, username prometheus_exporter, the file-provided api_key and login_options.user_info=false. Never persist this exchange.
2. system.info with params []. Retain version and physical total only.
3. core.subscribe with params ['reporting.realtime:{"interval":2}']. Keep at most 30 notifications and at most 70 seconds acquisition including subscription acknowledgment. The full frame limit is 1 MiB and selected event fields limit 128 KiB. Require exactly one interface for this initial comparison; abort if the observation contains multiple interfaces rather than silently selecting one. Keep its real identifier only in memory. Alias it consistently before persistence.
4. core.unsubscribe with params [the connection-owned acknowledged ID]. Require null acknowledgment, within five seconds reserved even on acquisition errors. After a stream failure, cleanup and terminate the window without chart queries.
5. Define A as floor(first event receipt epoch)+2 and B as floor(last event receipt epoch)-2. Require 0 < B-A <= 60 and at least two observations. Both are absolute UTC epoch seconds. Receipt time is an approximation of producer sampling; compare returned chart timestamps and mark unpaired samples explicitly.
6. reporting.netdata_get_data with the two positional parameters below. I is the in-memory interface name, A and B are the bounded integers above.

```json
[
  [
    {"name":"cpu","identifier":null},
    {"name":"memory","identifier":null},
    {"name":"interface","identifier":"I"},
    {"name":"arcsize","identifier":null}
  ],
  {"start":111,"end":222,"aggregate":true}
]
```

The literal I, 111 and 222 in the template must be replaced by I, A and B. Do not send unit/page to select a broad relative history. Aggregate true is explicit because the result schema requires min/mean/max aggregation objects; the comparison uses timestamped rows. Do not substitute aggregate false until its empty aggregation object versus the declared response schema has been resolved offline.

7. Call reporting.netdata_graph once for each of the following eight fixed names, sequentially, with params [NAME,{"start":A,"end":B,"aggregate":true}].

| NAME | Underlying chart |
| --- | --- |
| demanddatahitpercentage | truenas_arcstats.ddh_p |
| demandmetadatahitpercentage | truenas_arcstats.dmh_p |
| demanddatahitspersecond | truenas_arcstats.ddhit |
| demanddataiohitspersecond | truenas_arcstats.ddioh |
| demanddatamissespersecond | truenas_arcstats.ddmis |
| demandmetadatahitspersecond | truenas_arcstats.dmhit |
| demandmetadataiohitspersecond | truenas_arcstats.dmioh |
| demandmetadatamissespersecond | truenas_arcstats.dmmis |

8. Close the socket. The maximum is 13 client RPC requests including authentication and owned cleanup: login, system, subscribe, unsubscribe, one batch and eight single graphs. There are no pool, interface.query, chart-discovery or administrative requests from this harness. Internal calls performed by reporting itself are described below.

## Bounds and source transformations

Each chart reply is capped at 1 MiB before full decoding; total received chart replies at 9 MiB. After parsing, retain at most 300 rows per chart and 32 dimensions per row, with at most 12 returned charts across all calls. Reject out-of-window data, extra charts, duplicate or unknown columns, nonfinite numbers or unexpected types. A cap violation terminates the window. These are client limits. The server plugin asks Netdata for up to 2999 points and average grouping; the public schema has no points/dimension-selection parameter. Restricting A..B bounds requested time, while client rejection cannot prevent all upstream allocation. Empty or malformed replies are evidence of unavailable comparison and must not become zeros.

CPU maps to truenas_cpu_usage.cpu, memory to truenas_meminfo.available, interface to net.I and arcsize to truenas_arcstats.size. The interface plugin builds context using internal interface.query and chart inventory, and its returned sent column is made positive. GraphBase renames labels to legend, requests flip|null2zero|natural-points with average grouping and strips an all-zero last row. Therefore zero in chart history cannot establish raw nullness and the last missing row is not proof that no zero sample existed. Record all these transformations alongside residuals.

The batch GraphIdentifier schema only accepts an older literal list, which includes cpu, memory, interface and arcsize. New demand graph names are registered dynamically but excluded from that literal list. reporting.netdata_graph accepts a nonempty string and selects the registered plugin, providing the source-supported route for these names. Each selected plugin has uses_identifiers=False, so it selects a single chart.

Compare CPU chart percent/100, available/ARC bytes directly, and interface kilobits/s times 1000/8 against the event's normalized integer bytes/s. Account for source round(), time alignment and average grouping. Keep local acquisition start/end, UTC receipts and returned chart timestamps. There is no exact per-event producer timestamp in this event payload. Agreement demonstrates consistency through a shared chart pipeline. A mismatch without matched sample windows remains unresolved.

## Public API limits and separate raw evidence

No reviewed public reporting method exposes raw cumulative /proc/stat, interface byte counters or ARC demand counters. The per-second ARC graphs have already passed through Netdata's incremental processing. Dividing those series cannot recover original cumulative counters. Public plugins above also lack negotiated link speed, raw total-memory history, installed engine version and dimension algorithm/null metadata.

The source has netdata.get_chart_details, netdata.get_chart_metrics and netdata.get_all_metrics under a private NetdataService. The first two also declare private=True. get_chart_metrics additionally injects null2zero. The Python connector's get_info is an internal class method, not a public RPC. None is included in this permission request. No extra administrator identity or role broadening is proposed.

To close the remaining gaps, a separate operator-approved local diagnostic export could collect two or more synchronized readings of /proc/stat, MemTotal/MemAvailable from /proc/meminfo, the selected interface's rx_bytes/tx_bytes and speed under /sys/class/net/I, and size plus demand_data_hits, demand_data_iohits, demand_data_misses, demand_metadata_hits, demand_metadata_iohits and demand_metadata_misses from /proc/spl/kstat/zfs/arcstats. A maximum of 30 samples at two-second spacing within 70 seconds suffices for this initial window. Require the same interface and timestamps as the paired window; counter reset or zero denominator makes that interval unavailable. Preserve only selected numbers and aliases. This needs separate shell/diagnostic authorization and an inspected collector before execution.

The pinned netdata/utils.py fixes the internal endpoint to http://127.0.0.1:6999/api and NETDATA_UPDATE_EVERY=2. connector.py uses GET /api/v1/info for engine info and GET /api/v1/charts for chart metadata. A separately approved operator-local metadata export can therefore make exactly these two loopback GET requests on the appliance, once each, with five-second per-request timeout, 256 KiB info cap and 4 MiB charts cap. The charts endpoint enumerates inventory internally; retain only truenas_cpu_usage.cpu, truenas_meminfo.total/available, net.I, net_speed.I, net_operstate.I and the nine selected ARC charts (size plus the eight listed above). Retain dimension IDs, algorithm, multiplier/divisor, update interval and availability, plus engine version. No direct network access from the Mac to port 6999 or exposed listener is proposed. These local HTTP reads require separate operator shell/diagnostic authorization; they are not public middleware calls. If access is denied, stop. Do not alter service configuration or credentials.

A short independently inspected operator-local collector must combine the two metadata reads with the bounded file reads above, filter names and sanitize before export. That collector is still to be prepared and validated offline if this evidence path is chosen. A metadata snapshot and raw counter export remain separate authorization items. Stop at public chart consistency if that evidence is unavailable.

## Source references

All 35 downloaded reporting implementation/schema files were verified against Git blob hashes in the pinned tree. The role, schema and mapping conclusions come from these files.

- [Public methods and roles](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/plugins/reporting/graphs.py)
- [Reporting argument/result models](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/api/v25_04_2/reporting.py)
- [Plugin registration and processing](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/plugins/reporting/netdata/graph_base.py)
- [Graph-to-chart mappings](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/plugins/reporting/netdata/graphs.py)
- [Private Netdata service](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/plugins/reporting/rest.py)
- [Netdata connector](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/plugins/reporting/netdata/connector.py)
- [System info declaration](https://github.com/truenas/middleware/blob/244b717370fe35fb9eefc3096acfbc25f12b57b6/src/middlewared/middlewared/plugins/system/info.py)

## Harness implementation

The explicit Cargo example public_chart_comparison implements this scope with synthetic TLS regressions. Its first authorized window stopped during cleanup after four calls, before chart queries. See public-chart-comparison-results.md for partial evidence and the notification-order regression.
