# Public chart comparison harness and results

The operator authorized one 13-call public chart window on November 14, 2026, using the dedicated file credential. The harness is an explicit Cargo example calling a dedicated diagnostic module. Normal exporter scheduling and its method allowlist are unchanged. Root release 0.5.0 introduces the diagnostic; Cargo package version remains 0.1.0.

## Harness

Run the example only with explicit appliance authorization and the established file-based configuration. It takes a single new output path and creates it exclusively. It performs no discovery, retry or reconnect. Production authentication/TLS, system parsing, event decoding and protocol error categorization are reused. Only fixed chart request sites occur in the private session implementation. Request count, acquisition/cleanup/total deadlines, frame/selected-field/chart-byte limits, chart shapes, timestamps, labels and finite values are checked. The example persists selected measurements and aliases, omitting raw server errors, source identities and subscription IDs. Validated partial charts survive a later failure. Early authentication or version failure can leave an empty output file and a bounded error.

Synthetic TLS tests cover the complete 13-call sequence, chart sanitization/null preservation, invalid event cleanup and skipped queries. Pure validation cases reject duplicate/unknown charts, bad timestamps, repeated times, excessive row counts, empty data and unknown columns. Synthetic event spacing is accelerated to 250 ms for these tests. The live subscription interval remains exactly two seconds.

## First attempt

Version 25.04.2.6 and physical total 63,093,960,704 bytes were verified. Thirty selected notifications were received. The attempt stopped with a protocol category during its fourth RPC, core.unsubscribe, after 59.019 seconds. The close frame was sent successfully; the harness did not observe the unsubscribe acknowledgment. No chart query was sent. The selected partial report is preserved in evidence/public-chart-comparison-attempt-1-2026-11-14.json.

Pinned common/event_source/manager.py terminates the app subscription by sending notify_unsubscribed during unsubscribe. The diagnostic initially treated every notification as an update, while the production cleanup path accepts termination notifications while waiting for the RPC acknowledgment. The harness now accepts a matching, error-free notify_unsubscribed during cleanup and still requires the null RPC reply. The loopback fixture reproduces that notification-before-reply ordering, and both success and malformed-event cleanup tests pass with it. This source-supported explanation fits the observed cleanup failure; the first attempt did not persist raw notifications to prove which frame triggered it.

An additional window requires separate approval because the original plan specified one window without automatic retry.

## Separately authorized corrected window

The operator explicitly approved one corrected window. It completed all 13 RPC requests in 59.072 seconds, received 30 realtime observations, acknowledged owned unsubscribe and sent the socket close successfully. system.info reported exactly 25.04.2.6 and physical total 63,093,960,704 bytes. Event receipt epochs span 23:11:30.312644 through 23:12:29.007491 UTC on November 14. These epoch estimates use the initial wall clock plus monotonic elapsed time. Nine chart replies totaled 20,326 bytes and produced the expected 12 graphs. Basic charts had 56 rows each; most ARC charts had 55 rows after the source's terminal-zero-row removal. The two separately approved attempts totaled 17 client RPCs, four plus thirteen.

Evidence is in [the corrected selected report](evidence/public-chart-comparison-corrected-2026-11-14.json) and [the computed residual summary](evidence/public-chart-comparison-summary-2026-11-14.json). Event fields are production-decoder conversions. Chart records preserve the validated timestamped history and legend, with interface identity replaced by interface-0 and unrelated response fields omitted.

## Receipt-aligned comparison

For each event whose receipt epoch lies within the requested chart start/end, select the chart row with smallest absolute timestamp difference (first row on a tie). Reject a pairing if the difference exceeds two seconds or either selected value is null. This yields 28 pairs for each row below. Use event minus converted chart value for the residual. CPU chart percent divides by 100, available/ARC bytes use round(abs(value)), interface kilobits/s use round(abs((value/8)*1000)), and ARC percentages use round(abs(value))/100, matching the source adapter. Python round is used. No zero is substituted for a missing comparison.

| Quantity | Exact equal pairs | Median absolute residual | Maximum absolute residual |
| --- | --- | --- | --- |
| Aggregate CPU ratio | 24 / 28 | 0 | 0.01 |
| Available memory bytes | 0 / 28 | 2,134,576 | 14,670,016 |
| ARC size bytes | 0 / 28 | 4,136 | 162,128 |
| RX bytes/s | 6 / 28 | 122.5 | 804 |
| TX bytes/s | 6 / 28 | 639 | 30,228 |
| ARC data hit ratio | 28 / 28 | 0 | 0 |
| ARC metadata hit ratio | 28 / 28 | 0 | 0 |

For all 13 CPU dimensions together, 294 of 364 receipt-aligned pairs were exactly equal; the maximum ratio residual was 0.06. Most selected chart timestamps differed from receipt epochs by -0.498 to +0.479 seconds. The percentage charts' final zero row had been removed, so one final pairing used a row up to 0.984 seconds earlier. These offsets measure chart time versus local receipt, not source acquisition latency. Public history is averaged/natural-point output and the event takes latest allmetrics values. Nearest receipt-time pairing does not prove an identical underlying sample or an exporter conversion defect. Therefore these residuals are observations without an acceptance threshold.

## ARC conclusion and remaining evidence

Both public percentage histories returned zero in every row, agreeing with the event zero ratios. Data hit-rate history ranged 10.016743–204.7157/s and metadata hit-rate history 0–1361.2409/s. I/O-hit and miss histories returned zero throughout. This confirms the same zero-percentage behavior through the public history path with positive hit-rate observations. Source null2zero and chart algorithms still prevent attribution to raw nullness, measured zeros or algorithm transformation. Dividing these histories does not reconstruct independent cumulative or interval hit fractions.

The supported public chart path and selected read permissions succeeded with the dedicated credential. This does not inventory minimum effective roles. Independent raw counter/metadata comparison, combined scheduled-daemon acceptance and production rollout remain open. No private method, shell, role change or infrastructure edit was included in either window.
