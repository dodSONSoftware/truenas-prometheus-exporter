# Process-test startup port race — November 14, 2026

Release 0.8.6 fixes the reproduced intermittent early exit in the synthetic process tests. This is a test-harness change; production code, listener configuration, API allowlists, dependencies and deployment remain unchanged.

## Root cause and reproduction

The old fixtures asked the OS for an ephemeral loopback TCP port, closed that listener, launched the exporter on the now-unreserved address, and immediately began HTTP probes. Port selection did not reserve the child's future listener. A competing local socket could claim the address before the child bound it, and a successful TCP connection did not prove it reached the intended child.

Adding the child's exit status and stderr exposed the previously hidden cause:

```text
exporter exited prematurely (exit status: 1): Error: cannot bind exporter listener

Caused by:
    Address already in use (os error 48)
```

Thirty isolated malformed-notification runs passed. The normal parallel process suite reproduced the error on run seven. An overlapping two-run reproduction also failed on pair six in the other redaction test, with the same bind error. This establishes the startup bind collision; it does not identify the particular competing socket or independently attribute the older configuration-test ConnectionReset report.

## Fix and regression

Shared code in application/tests/support/mod.rs now starts the synthetic child and waits for its existing info-level HTTP startup log line, emitted after the actual listener bind. HTTP probes begin only after that child confirms startup, rather than probing an address while it is unreserved.

Because a parent-selected port cannot be atomically handed to the unchanged executable, startup may select another ephemeral address only when the child exits with code 1 and the specific cannot-bind / Address-already-in-use error. There are at most three launches within one five-second startup deadline. Spawn failures, configuration errors, other exits, oversized startup output and timeout fail immediately; no protocol, metrics, redaction, readiness or shutdown assertion is retried. Every failed launch is awaited. kill_on_drop remains enabled for interrupted startup. Startup stdout is capped at 64 KiB and returned to callers so the existing trace-redaction assertions still inspect it.

The helper is used by both redaction process cases, the environment-Unicode process case and the combined realtime process case. It is not linked into the runtime binary. The production port-zero rejection contract is preserved.

The focused regression uses real loopback sockets and the actual mock exporter executable. It deliberately occupies the first candidate, verifies recovery on a different address and a successful health response, occupies every candidate to require failure after exactly three launches, and supplies an invalid logging filter to require immediate failure after one launch. With collision handling disabled, the regression failed with the expected Address-already-in-use error; restoring the fix made it pass.

## Verification and limits

Sixty process-suite runs in 30 concurrent pairs passed, totaling 300 test executions. This repeats the conditions that exposed the old failure; any failed run would have stopped the experiment. The unchanged split-status-line test was excluded from this repetition, and remains part of the ordinary full suite. No long sustained profile or appliance interaction was performed.

The full native scripts/check.sh passed formatting, strict Clippy, 84 Rust tests and 18 Python tests. Twenty configuration-suite repetitions passed (220 tests). The 18 affected process/config/realtime_process tests also passed in release mode on pinned Rust 1.88 linux/amd64 with runtime networking disabled. The Linux compile initially lacked the sanitized fixture mount; the correctly mounted run passed. The pinned runtime image build passed. Full details are recorded in validation.md. All fixtures use generated credentials and loopback endpoints. No service, dashboard, credential, role or infrastructure configuration was changed. The installed Linux exporter remains image 0.8.0. Finite repetitions do not prove that every historical failure or future scheduling issue has been eliminated.
