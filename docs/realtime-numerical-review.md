# Realtime numerical source review

Reviewed November 14, 2026. Offline source-to-wire review is complete; independent live chart/numerical acceptance remains open. No appliance call, configuration change or additional permission was used. Runtime behavior is unchanged. This review does not certify ARC ratios as operational cache-efficiency measurements.

## Evidence and method

The authority is [TrueNAS middleware TS-25.04.2.6](https://github.com/truenas/middleware/tree/244b717370fe35fb9eefc3096acfbc25f12b57b6), commit 244b717370fe35fb9eefc3096acfbc25f12b57b6. Public source was retrieved from GitHub; all 20 locally reviewed files were verified against Git blob IDs in that commit's tree. No upstream source is vendored or modified here. The four saved notifications establish event fidelity, not simultaneous source-chart agreement.

A temporary Python harness loaded pinned pure conversion functions with synthetic dictionaries/counters. It did not read procfs, run Netdata or invoke service methods. Python 3.9 initially failed to import newer type annotations; enabling deferred annotations allowed the full experiment to pass without altering function bodies or arithmetic. This was a source-level experiment, not a TrueNAS/Netdata runtime test.

Paths below are relative to src/middlewared/middlewared/ unless marked chart; chart files are under src/freenas/usr/lib/netdata/python.d/ in the same pinned tree.

## Source trace

| Quantity | Source operation | Exporter interpretation and limit |
| --- | --- | --- |
| CPU | utils/metrics/cpu_usage.py derives /proc/stat deltas, subtracts idle and iowait, divides by total delta and rounds to two decimals. Chart truenas_cpu_usage.chart.py uses absolute dimensions. Event adapter passes chart usage through, defaulting missing/null to zero. | Percent / 100 once. Load average and iowait are not this metric. Chart transport can affect fractional precision. Aggregate need not equal an unweighted mean of rounded cores. |
| Physical memory | utils/memory.py multiplies Linux MemTotal/MemAvailable kB by 1024. Chart truenas_meminfo.chart.py declares Bytes. Event normalizes with absolute value and integer rounding. | Available bytes pass through, validated against positive total. System info remains the canonical total metric. Available is not ARC-free or ARC-available; 1 - available/total alone does not diagnose pressure. Missing available can already be zero upstream. |
| Network speed | realtime_reporting/ifstat.py divides chart speed by 1000, takes absolute value and rounds, yielding event Mbps. | Event Mbps x1,000,000 once. Chart value 1,000,000 becomes event 1000 then 1e9 bits/s. This verifies adapter arithmetic, not the installed collector's raw speed units. Upstream rounding loses sub-Mbps precision. |
| Network rates | Same adapter interprets chart values as kilobits/s, multiplies by 1000/8, takes absolute value and rounds to integer bytes/s. | Already byte-rate gauges; no second interval or bit conversion. Negative transmit chart values are normalized upstream. Integer rounding loses roughly half a byte/s, subject to floating-point/tie behavior. |
| Link state | Missing/null up dimension defaults to zero; bool false becomes DOWN. | Exporter recognizes supplied UP/DOWN but cannot distinguish upstream missing-data DOWN. It omits down-link speed/rates even if supplied as zero. A link-state zero does not independently prove physical failure. |
| ARC size | utils/metrics/arcstat.py reads size from procfs; chart declares Bytes; event applies absolute value and integer rounding. | Required nonnegative bytes pass through. Range validation cannot recover availability when upstream defaults size to zero. |
| ARC demand hit percentages | ARC utility computes ordinary hits / (ordinary hits + I/O hits + misses) from cumulative counters as read, without locally differencing them. Chart truenas_arcstats.chart.py registers these derived percentages as percentage-of-incremental-row. Event normalizes/rounds and defaults missing/null to zero. | Received percent /100 once. Ordinary hits exclude the separately reported I/O-hit component. The chart algorithm applied to precomputed percentages is a static semantic concern requiring installed-engine verification. Do not infer lifetime/interval cache efficiency from field names or zeros. |

The shared adapter behavior is in plugins/reporting/realtime_reporting/utils.py: safely_retrieve_dimension returns the supplied default for missing keys/null dimensions; normalize_value defaults to absolute value and Python round. These limitations affect more than ARC. The exporter itself does not insert missing-field zeros, but it cannot reconstruct provenance already lost by the producer.

## Executed synthetic experiments

These are constructed inputs, not appliance captures:

| Input | Observed pinned function result |
| --- | --- |
| CPU active delta 20, idle 60, iowait 20, total 100 | 20.0 percent; exporter conversion 0.2 |
| Up-link chart speed 1,000,000 | Event 1000; exporter conversion 1e9 bits/s |
| Receive 8.004 and transmit -16.003 chart kilobits/s | Event 1000 and 2000 bytes/s after absolute value/rounding |
| Selected ARC chart percentage missing / null / 95.6 | Event 0 / 0 / 96; exporter ratios 0 / 0 / 0.96 |
| Memory charts absent | Event available 0; total also 0 and rejected by exporter |
| Interface charts absent | Event labels synthetic interface DOWN |
| ARC cumulative data counters: ordinary hits 90, I/O hits 5, misses 5 | Utility ordinary hit percentage 90, separate I/O-hit percentage 5 |
| ARC cumulative metadata counters: ordinary hits 80, I/O hits 10, misses 10 | Utility ordinary metadata hit percentage 80 |

The experiment did not execute the Netdata percentage-of-incremental-row engine. Potential distortion of already calculated percentages is an inference, not a reproduced runtime defect. Missing chart/key/null is one proven route to event zeros; it is not a diagnosis of the captured zeros. No exporter correction is justified by these experiments alone.

## Captured baseline assessment

All four saved notifications convert correctly under the current contract: available/ARC bytes and byte rates are retained; 1000 Mbps becomes 1e9 bits/s; CPU 0/0/9/8 percent becomes 0/0/0.09/0.08. Selected ARC percentages are zero throughout. The captures contain no chart availability, engine version, calculation history or simultaneous independent measurement. Neither the cause of ARC zeros, actual link speed nor UI/Graphite equivalence is established. Rewriting explicit zeros as absent would invent unavailable provenance.

See [capture provenance](../fixtures/truenas-25.04.2.6/README.md) for values/times and production-worker replay. No field-validation, freshness, required-group, API-allowlist or default changes follow from this review.

## Remaining acceptance procedure

1. Obtain operator-supplied timestamped Reporting views for CPU, memory, link speed/rates and selected ARC dimensions, or separately authorize a bounded source-chart comparison with known API method, role and response limits. Event authorization does not authorize arbitrary chart APIs, shell commands or private Netdata methods. No broader credential is proposed.
2. Match version, time window, sampling interval, averaging and chart identity. Use real interface identity only in memory; save aliases and a minimal sanitized report. Historical Graphite examples in the handoff are not contemporary paired evidence.
3. Compare CPU percentage points with ratio x100, memory with consistent binary/decimal units, signed network kilobits/s with absolute bytes/s x8/1000, and speed with event Mbps. Account for source rounding and differing windows; do not invent a universal tolerance.
4. For ARC, inspect chart presence/nullness and dimension IDs versus display names. Record installed chart algorithm/runtime version and compare independently derived counter deltas with a declared denominator/interval. UI agreement alone cannot prove correctness if both paths share the same algorithm. Do not induce faults or change appliance configuration.
5. Close only comparisons supported by paired evidence. Leave unavailable ARC semantics unresolved. Integrated sustained/outage/resource checks can proceed independently with controlled loopback fixtures; no live daemon or production deployment is implied.

## Additional pinned source identities

| File | Git blob SHA-1 |
| --- | --- |
| chart truenas_arcstats.chart.py | fbc27441f48d2ec6daba9c93bbaffe53972b2fb5 |
| chart truenas_cpu_usage.chart.py | f4cf69900fd9f4794a8798f250140d590b080f2a |
| chart truenas_meminfo.chart.py | f3e51b80f91a3a4a97b7bfc846594c7fe259cd1b |
| utils/metrics/arcstat.py | 9b457119d991f38ed3e62080c976d50a8abc3c7f |
| utils/metrics/cpu_usage.py | 5734a9bf43e1ace699ec438b5f71840d2b23b139 |
| utils/memory.py | a2509e9a922392f0bf9710a7ef57a9868f2455d3 |

These bind inspected files to the pinned tree, not appliance-installed files or a separate release attestation.
