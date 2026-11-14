# Compatibility and evidence

Target: TrueNAS SCALE 25.04.2.x, initially 25.04.2.6. Initial implementation used offline mocks; subsequently authorized transport-only testing through memory-alpha.home.lab is recorded in memory-alpha-integration.md. No authenticated appliance API query has run yet. Historical values in the handoff are not current observations.

## Versioned evidence

- Authentication: https://github.com/truenas/middleware/blob/TS-25.04.2.6/src/middlewared/middlewared/api/v25_04_2/auth.py defines API_KEY_PLAIN, username, api_key, and SUCCESS/AUTH_ERR/EXPIRED/OTP_REQUIRED/REDIRECT results.
- Protocol: https://github.com/truenas/api_client/blob/TS-25.04.2.6/truenas_api_client/__init__.py implements JSON-RPC correlation, core.subscribe/core.unsubscribe, collection_update and notify_unsubscribed. Tag is the compatibility baseline; do not infer behavior from master.
- Official docs: https://api.truenas.com/v25.04.2/api_methods_auth.login_ex.html and https://api.truenas.com/v25.04/api_events_reporting.realtime.html. Some hosted pages returned fetch errors during review. Indexed reporting evidence was labeled 25.04.1; it is not a captured 25.04.2.6 payload.
- Reference only: https://github.com/Unknowlars/truenas-scale-api-prometheus-exporter targets 25.10.2. Do not copy its authentication, units, port, or generic discovery behavior.

All checked-in JSON fixtures are SYNTHETIC, derived from the specification and versioned evidence. They prove local behavior, not appliance compatibility. No complete captured pool or realtime payload was supplied. Captured, sanitized 25.04.2.6 fixtures remain a separate acceptance gate.

M2 implements authentication and pool.query only. Runtime appliance version discovery, system polling, and realtime subscriptions are deferred to M3; configuring a 25.04.2.x endpoint is currently an operator obligation, not a runtime-verified claim. /api/current is configurable and may change following an appliance upgrade. Unsupported schema must fail validation, never produce default zeros.

Before deployment, separately authorize validation of endpoint/certificate identity, API version, account roles for every method/event, and real response envelopes. Automated tests/CI remain offline. The explicit check-transport command can test an operator-authorized endpoint without credentials or API methods. Minimum account roles are not yet established. No read-only credential is created by the exporter.

Captured sanitized payloads belong in `fixtures/truenas-25.04.2.6/` at the repository root. That directory currently contains intake instructions only; it is not evidence of captured-fixture validation.

The operator reconfirmed TrueNAS 25.04.2.6 during proxy setup. Host/container verified WSS upgrade is established, but this does not independently verify backend version, account roles or method response compatibility. The dedicated account/key are not yet provisioned.

On November 13, the operator reconfirmed 25.04.2.6 and selected local macOS Docker Desktop for development. A screenshot of Credentials → Users shows no dedicated monitoring user among the three listed accounts. Account provisioning is being guided by the operator; no authenticated appliance capture has been authorized or performed in this continuation.
