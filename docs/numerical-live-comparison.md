# Bounded numerical comparison proposal

Prepared November 14, 2026. Operator authorization is required before execution under AGENTS.md and the appliance boundary. This document requests a specific observation window. It does not supply authorization.

## First window — event fidelity and operator Reporting comparison

Use the local Mac, verified WSS endpoint and existing dedicated file credential/CA. One target, one two-second reporting.realtime subscription, a 120-second total deadline including cleanup, at most 45 notifications, 1 MiB maximum frame and 128 KiB selected event payload. Authenticate once, call system.info once to verify 25.04.2.6 and physical total, then core.subscribe with exactly reporting.realtime:{"interval":2}. Always attempt core.unsubscribe for the acknowledged connection-owned ID within a five-second cleanup budget included in the deadline. Stop on version/auth/permission/shape failure. Do not retry the observation window automatically.

During the same window the operator records Reporting views for aggregate/per-core CPU, physical available/total memory, the selected interface's speed and RX/TX, and ARC size/data/metadata percentages. Record visible time range, timezone, units, averaging and interface alias. The operator opens the views using their normal session. The exporter credential is used only for the explicit methods above. A Reporting view backed by the same chart validates presentation agreement only.

Retain at most 45 selected rows and the minimal operator comparison record. Save UTC receipt time, monotonic offset, any producer timestamp, exact software version, metric values, units and aliases. Omit credentials, authentication frames, subscription IDs, real hostname/interface identity and unrelated payload. An absent chart/dimension is recorded as absent. Local receipt time is not producer sample time.

Compare event CPU/100 with exported ratio, bytes with bytes, speed Mbps times 1,000,000 with bits/s, and instantaneous RX/TX bytes/s directly. If the chart uses signed kbit/s, compare abs(chart)*1000/8 before upstream rounding. Match chart sampling and aggregation before calculating residuals. For a truly paired source sample, require the exporter conversion to agree within floating-point representation, and allow only the source's declared rounding. A view with unknown timing/averaging cannot close numerical accuracy. Do not use a single universal percentage tolerance.

## Second window — independent upstream evidence

ARC zero provenance requires source observations unavailable in the saved captures. Request operator-provided data from an approved observation mechanism before proposing new automated API or shell access. For a window of at most 120 seconds and 45 samples, obtain timestamped raw chart dimension values including nullness, dimension IDs, algorithms, update interval and installed Netdata version. Required charts are truenas_arcstats.size, ddh_p and dmh_p, plus corresponding hits, I/O hits and misses. For physical checks also obtain synchronized /proc/stat counters, MemTotal/MemAvailable, selected-interface byte counters and negotiated speed, and the ARC size plus six demand counters from arcstats. The operator must choose and approve the acquisition mechanism. This proposal does not invent a callable public method for these private/raw values.

For CPU use two matching counter snapshots, subtract idle and iowait from total delta and compare the documented producer result. Check counter resets and sample durations. For network divide nonnegative byte deltas by actual elapsed time, accounting for averaging and rounding. For memory multiply Linux kB by 1024; compare ARC size at the paired instant. For ARC calculate both cumulative ordinary-hit fractions and interval ordinary-hit fractions, with I/O hits included in each denominator. Zero denominator or reset makes that interval unavailable. Record which of these the installed chart actually computes before interpreting its percent. Never infer the denominator from the event rate names alone.

Closure requires recording chart availability separately from zero, explaining the installed percentage-of-incremental-row transformation, and identifying whether captured-style zero ratios are missing-data defaults, algorithm output or measured zero. If this evidence is unavailable, retain the explicit unresolved ARC acceptance gate. No load generation, scan, induced fault, role change or infrastructure change is part of either window.

## Separate combined daemon acceptance

After approval, run one local daemon for at most 180 seconds with pool/system poll 60 seconds, realtime interval two seconds, one target and loopback HTTP. Maximum four scheduled pool and four scheduled system reads on the polling socket (including connection gate), one realtime system gate, two authentications and one owned subscription. Abort the window on connection loss rather than allowing unbounded reconnect traffic. Sample local metrics/readiness every ten seconds for at most 18 samples. Cleanup includes termination, owned unsubscribe and verification that the local process exited. A controller enforcing these limits must be prepared and inspected before live execution. Confirm the dedicated role separately using operator configuration evidence. Avoid permission testing by administrative writes.

## Pinned public API refinement

See [supported reporting API plan](reporting-api-comparison-plan.md) for the verified public chart methods, REPORTING_READ/system.info role declarations, exact 13-request bounded sequence and separate operator-local counter/metadata proposal. The public chart path applies null2zero and cannot establish ARC zero provenance by itself.
