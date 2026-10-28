# MEMORY ALPHA integration status

Endpoint: `wss://memory-alpha.home.lab/api/current`. Backend and proxy details, supplied by the operator: TrueNAS SCALE 25.04.2.6 at 10.10.10.7, Caddy at 10.10.10.2. This integration test is explicitly authorized for read-only access; infrastructure changes remain prohibited.

## Verified in this run

- HTTPS hostname/certificate verification succeeds (HTTP 302 to the UI).
- The Rust `check-transport` command completes a verified WSS upgrade (HTTP 101) from the Mac.
- The actual linux/amd64 distroless exporter container completes the same verified WSS upgrade using a public CA certificate mounted read-only.
- Negative test: without the private CA configured, the container rejects the TLS connection.
- No authentication frames or JSON-RPC method calls were sent to MEMORY ALPHA. The command closes the WebSocket after upgrading.

Two Caddy root certificates were present in the macOS system keychain. Independent Rust checks rejected the stale root and accepted this one:

`SHA256 9da517d44e10e57108742ff9becaf8d477d1f9f3b745266c35f49e17139a607f`

The public certificate was exported from the trusted keychain, validated against the endpoint, and copied outside Git to `/Users/randydodson/.config/truenas-exporter/caddy-root-ca.pem`. The certificate file is public, user-owned, mode 0644. No CA was added to a trust store and no Caddy/TrueNAS configuration was read or changed through administrative access.

The Rust client uses Mozilla roots plus the explicitly supplied private CA, not macOS Keychain trust. Caddy handles its own upstream TrueNAS certificate verification; that infrastructure configuration is operator-reported and was not modified by this test.

## Authentication and pool.query: pending

The operator selected `prometheus_exporter` and `/Users/randydodson/.config/truenas-exporter/memory-alpha-api-key`, but explicitly states the account/key are not created yet. No placeholder API-key file was created, and no administrator credential was substituted.

- WebSocket transport: PASS on host and container.
- WebSocket authentication: NOT ATTEMPTED; dedicated credentials not provisioned.
- Live `pool.query`: NOT ATTEMPTED.
- Live API methods verified: none yet. Offline tests exercise auth.login_ex and pool.query.
- Appliance version: operator-confirmed 25.04.2.6; not independently queried by this exporter yet.
- Captured API fixtures: none; do not label synthetic fixtures as captured.

## Operator provisioning and next test

1. Create the dedicated account and API key through the supported TrueNAS interface. Restrict it to the required read-only access; do not use a full-administrator account/key. Exact role mapping on this appliance still needs verification. The current data method is pool.query; auth.login_ex establishes the session.
2. Save the key directly outside Git at the selected path, owned by the macOS user with mode 0600. Do not put the key in chat, environment values, shell command arguments/history, or logs. Keep the CA separate.
3. Confirm the credentials are provisioned. Only then perform authenticated login, inspect the typed response_type, and call pool.query read-only. Permission failures must not be bypassed by broadening to an administrator credential.
4. Save only reviewed sanitized response fixtures under fixtures/truenas-25.04.2.6/, including provenance. Never commit auth requests/responses or raw identifying payloads.
5. Repeat authenticated collection in the container with readable secret/CA mounts before claiming container API success. A mode-0600 file owned by the Mac user is not automatically readable by UID 10001 inside Docker. Validate actual mount ownership; preserve restrictive real-secret permissions rather than making the key world-readable. No credential ownership has been changed in this run.
6. Proceed with M3 version gating, system sources and realtime contracts after authenticated API compatibility is established.

## Transport-only commands (no API key needed)

Host:

```sh
TRUENAS_WS_URL=wss://memory-alpha.home.lab/api/current \
TRUENAS_CA_CERT_FILE="$HOME/.config/truenas-exporter/caddy-root-ca.pem" \
cargo run --locked --manifest-path application/Cargo.toml -- check-transport
```

Local Docker Desktop (build the validation image first):

```sh
docker --context desktop-linux run --rm --platform linux/amd64 \
  --read-only --cap-drop ALL --security-opt no-new-privileges:true \
  --mount "type=bind,src=$HOME/.config/truenas-exporter/caddy-root-ca.pem,dst=/run/secrets/memory-alpha-caddy-root-ca.pem,readonly" \
  --env TRUENAS_WS_URL=wss://memory-alpha.home.lab/api/current \
  --env TRUENAS_CA_CERT_FILE=/run/secrets/memory-alpha-caddy-root-ca.pem \
  local/truenas-rust-exporter:multi-target-validation check-transport
```

These explicitly scoped commands are manual live tests and never part of CI. They do not authenticate, call API methods, or confirm pool telemetry.
