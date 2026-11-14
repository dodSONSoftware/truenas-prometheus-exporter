# Compatibility and evidence

Target: TrueNAS SCALE 25.04.2.x, initially 25.04.2.6. Initial implementation used offline mocks; subsequently authorized transport and authenticated pool checks through memory-alpha.home.lab are recorded in memory-alpha-integration.md. Native macOS and local Docker checks passed on November 13, 2026 (November 14 UTC). Historical values in the handoff are not current observations.

## Versioned evidence

- Authentication: https://github.com/truenas/middleware/blob/TS-25.04.2.6/src/middlewared/middlewared/api/v25_04_2/auth.py defines API_KEY_PLAIN, username, api_key, and SUCCESS/AUTH_ERR/EXPIRED/OTP_REQUIRED/REDIRECT results.
- Protocol: https://github.com/truenas/api_client/blob/TS-25.04.2.6/truenas_api_client/__init__.py implements JSON-RPC correlation, core.subscribe/core.unsubscribe, collection_update and notify_unsubscribed. Tag is the compatibility baseline; do not infer behavior from master.
- Official docs: https://api.truenas.com/v25.04.2/api_methods_auth.login_ex.html and https://api.truenas.com/v25.04/api_events_reporting.realtime.html. Some hosted pages returned fetch errors during review. Indexed reporting evidence was labeled 25.04.1; it is not a captured 25.04.2.6 payload.
- Reference only: https://github.com/Unknowlars/truenas-scale-api-prometheus-exporter targets 25.10.2. Do not copy its authentication, units, port, or generic discovery behavior.

Synthetic fixtures in application/tests/fixtures/ remain explicitly synthetic. The root fixtures/truenas-25.04.2.6/ directory now contains genuine sanitized pool.query envelopes captured independently from the Mac and local Docker Desktop, plus provenance. Offline replay covers the production client, collector and metric encoding. Realtime and other API methods remain unverified.

Normal collection implements authentication and pool.query only. M3-A additionally provides an explicit, offline-tested system.info preflight; see system-contract.md. Runtime appliance version discovery, system polling, and realtime subscriptions are deferred to M3; configuring a 25.04.2.x endpoint is currently an operator obligation, not a runtime-verified claim. /api/current is configurable and may change following an appliance upgrade. Unsupported schema must fail validation, never produce default zeros.

Before deployment, separately authorize validation of endpoint/certificate identity, API version, account roles for every method/event, and real response envelopes. Automated tests/CI remain offline. The explicit check-transport command can test an operator-authorized endpoint without credentials or API methods. Minimum account roles are not yet established. No read-only credential is created by the exporter.

Captured sanitized payloads live in `fixtures/truenas-25.04.2.6/`. Its README records sanitization and version provenance. The operator confirmed 25.04.2.6 through the UI and provisioned a dedicated account with Readonly Admin. Successful pool.query establishes permission for this method; it does not establish the minimum role or test whether writes are denied. No other appliance methods were called, including system/version discovery.

M3-A derives the system.info selected-field contract and current-API registration from official tag TS-25.04.2.6 at revision 244b717370fe35fb9eefc3096acfbc25f12b57b6. This is source-derived compatibility evidence; there is no captured system response or observed system.info authorization yet. The daemon still does not automatically discover/gate the version or publish system metrics.
