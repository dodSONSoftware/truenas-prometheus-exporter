# Public chart comparison harness and results

The operator authorized one 13-call public chart window on November 14, 2026, using the dedicated file credential. The harness is an explicit Cargo example calling a dedicated diagnostic module. Normal exporter scheduling and its method allowlist are unchanged. Root release 0.5.0 introduces the diagnostic; Cargo package version remains 0.1.0.

## Harness

Run the example only with explicit appliance authorization and the established file-based configuration. It takes a single new output path and creates it exclusively. It performs no discovery, retry or reconnect. Production authentication/TLS, system parsing, event decoding and protocol error categorization are reused. Only fixed chart request sites occur in the private session implementation. Request count, acquisition/cleanup/total deadlines, frame/selected-field/chart-byte limits, chart shapes, timestamps, labels and finite values are checked. The example persists selected measurements and aliases, omitting raw server errors, source identities and subscription IDs. Validated partial charts survive a later failure. Early authentication or version failure can leave an empty output file and a bounded error.

Synthetic TLS tests cover the complete 13-call sequence, chart sanitization/null preservation, invalid event cleanup and skipped queries. Pure validation cases reject duplicate/unknown charts, bad timestamps, repeated times, excessive row counts, empty data and unknown columns. Synthetic event spacing is accelerated to 250 ms for these tests. The live subscription interval remains exactly two seconds.

## First attempt

Version 25.04.2.6 and physical total 63,093,960,704 bytes were verified. Thirty selected notifications were received. The attempt stopped with a protocol category during its fourth RPC, core.unsubscribe, after 59.019 seconds. The close frame was sent successfully; the harness did not observe the unsubscribe acknowledgment. No chart query was sent. The selected partial report is preserved in evidence/public-chart-comparison-attempt-1-2026-11-14.json.

Pinned common/event_source/manager.py terminates the app subscription by sending notify_unsubscribed during unsubscribe. The diagnostic initially treated every notification as an update, while the production cleanup path accepts termination notifications while waiting for the RPC acknowledgment. The harness now accepts a matching, error-free notify_unsubscribed during cleanup and still requires the null RPC reply. The loopback fixture reproduces that notification-before-reply ordering, and both success and malformed-event cleanup tests pass with it. This source-supported explanation fits the observed cleanup failure; the first attempt did not persist raw notifications to prove which frame triggered it.

An additional window requires separate approval because the original plan specified one window without automatic retry.
