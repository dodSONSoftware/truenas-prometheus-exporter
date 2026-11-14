# MEMORY ALPHA integration status

Endpoint: `wss://memory-alpha.home.lab/api/current`. Backend and proxy details, supplied by the operator: TrueNAS SCALE 25.04.2.6 at 10.10.10.7, Caddy at 10.10.10.2. This integration test is explicitly authorized for read-only access; infrastructure changes remain prohibited.

## Authorized realtime preflight — November 14, 2026 UTC

The operator explicitly approved check-realtime against wss://memory-alpha.home.lab/api/current from macOS and local Docker Desktop using the existing dedicated credential file. Both invocations completed successfully and printed only the fixed confirmation:

> Verified WSS, system compatibility and fresh CPU/memory/network/ARC groups; owned realtime subscription closed

The command performed verified TLS/hostname checks, typed auth.login_ex, system.info validation against the supported 25.04.2.x family, core.subscribe for exactly reporting.realtime:{"interval":2}, observation of fresh valid CPU/memory/network/ARC snapshots, and core.unsubscribe for its own acknowledged identifier. Successful completion includes the null unsubscribe acknowledgement and WebSocket close. No pool query, arbitrary subscription or administrative method was sent.

The Mac ran the repository binary through cargo run --quiet --locked. Docker Desktop used desktop-linux, linux/amd64 and local/truenas-rust-exporter:realtime under UID/GID 10001:10001, read-only root, dropped capabilities, no-new-privileges and a 128 MiB memory limit. That limit was a run constraint, not a measured maximum-target resource result.

The existing Mac key remained mode 0600. A temporary Docker volume received the key through stdin, with mode 0400 and ownership 10001:10001, and was mounted read-only. The CA was also mounted read-only. The temporary helper/probe containers and secret volume were removed; subsequent inventories were empty for their names. Credentials, raw responses, source identities, event payloads and subscription IDs were not printed or saved. No captured realtime fixture was created by these preflights.

This establishes successful read subscription and owned unsubscribe with this credential, plus live acceptance of the selected group schemas. It does not independently inventory its role list, establish minimum privilege or denial of writes, record the current exact patch version beyond the supported-family check, compare UI/Graphite values, or validate a long-running integrated daemon. Existing system fixtures retain their earlier exact 25.04.2.6 provenance. Captured realtime envelopes and full enabled-mode resource/outage acceptance remain pending. No infrastructure, live dashboard or production deployment was changed.

## Authorized system checks — November 14, 2026 UTC

The operator separately authorized read-only auth.login_ex and system.info checks against the configured WSS endpoint from the Mac and local Docker Desktop, using the existing dedicated credential file.

- The actual production check-system command passed on native macOS and the local linux/amd64 distroless image local/truenas-rust-exporter:system-preflight. Both verified TLS/hostname, typed authentication, selected system.info fields and 25.04.2.x compatibility. The command printed only fixed success text.
- A separate temporary Rust capture harness reused configuration/TLS/protocol/authentication helpers and the system parser. It requested system.info with params=[] once per capture, under 1 MiB frame/message bounds and a 30-second outer timeout. It sanitized successful replies in memory before writing anything; raw responses and authentication exchanges were never saved.
- The API-reported short version is 25.04.2.6 in both captures. This is an API observation through the verified configured endpoint, beyond the preceding operator-only version evidence. Automatic per-connection daemon version gating is still not implemented.
- Both Docker runs used UID/GID 10001:10001, read-only root, dropped capabilities, no-new-privileges and a 128 MiB memory limit. The key was provisioned over stdin into a temporary volume as mode 0400, owned by UID 10001 and mounted read-only. The CA was also mounted read-only. Temporary containers and secret volumes were removed, and final inventory checks found none remaining. The Mac key remains mode 0600.
- Full sanitized host/container system envelopes are saved beside the pool captures. The fixture README records local UTC output timestamps, aliasing, preserved measurement fields and limitations. Serial/model/product/manufacturer/timezone strings were replaced; the hostname is aliased, and license is null in both captures.
- Offline captured_system_envelopes_replay_through_preflight_offline replays each full envelope through the production check_system path over loopback TLS, changing only the mock request ID. It asserts API version, hostname alias, physical-memory bytes, fractional uptime and load values. The existing decoder accepted the genuine responses without changes.

These runs establish successful read authorization for system.info with this dedicated credential. The exact role assignment remains operator-reported; minimum privilege and denial of writes were not tested. No system.version call, subscription, administrative method, infrastructure edit or production deployment occurred. Future realtime methods require their own explicit authorization. M3-C system scheduling/version-gate/freshness work remains an offline implementation task.

## Earlier authenticated pool checks — November 13, 2026

The operator explicitly authorized read-only `auth.login_ex` and `pool.query` checks against the endpoint above from the Mac and local Docker Desktop, using the operator-provisioned credential file. The operator reported the dedicated account as Readonly Admin and reconfirmed software version 25.04.2.6 in the UI. No version method was called.

- Native macOS: verified TLS/hostname, typed SUCCESS authentication, pool.query with empty parameters, and pool decoder all passed; one pool returned.
- Docker Desktop `desktop-linux`, linux/amd64: the same checks passed in the pinned distroless runtime, under UID/GID 10001:10001, a read-only root filesystem, dropped capabilities, no-new-privileges and a 128 MiB memory limit.
- A temporary Rust probe reused Config, TLS roots, protocol login/request/response/authentication helpers and the pool collector, with 1 MiB WebSocket limits and a 30-second overall deadline. It did not start the HTTP exporter or exercise the production reconnect supervisor against the appliance. The offline replay test exercises the production Client instead.
- The Mac key remained user-owned mode 0600. A temporary Docker volume received its contents through stdin from a host process (no key in arguments, environment or logs). The helper set mode 0400 before transferring ownership to 10001:10001. The probe mounted that volume read-only. The temporary containers and secret volume were removed after each attempt. An initial helper attempt failed on chmod after ownership transfer; it was cleaned up, and the ordering was corrected before retrying.
- The CA was mounted read-only. No TLS verification was disabled. No appliance configuration or access controls were changed by these tests.
- Full pool response structures were sanitized in memory before saving. Original pool names, GUIDs, device identities and paths were not persisted. No authentication exchanges or raw responses were saved. See `fixtures/truenas-25.04.2.6/README.md` for exact transformations.
- Successful authentication and pool.query demonstrate this account can read pools; the breadth/minimum of its privileges and denial of writes were not tested. All other methods and realtime events remain outside this authorization.

TN-02 now has captured appliance evidence and offline replay. M3 can begin offline design/implementation; live version/system/event validation needs separately scoped operator authorization. Production deployment and dashboard changes remain unauthorized.

## Historical transport-only checks

- HTTPS hostname/certificate verification succeeds (HTTP 302 to the UI).
- The Rust `check-transport` command completes a verified WSS upgrade (HTTP 101) from the Mac.
- The actual linux/amd64 distroless exporter container completes the same verified WSS upgrade using a public CA certificate mounted read-only.
- Negative test: without the private CA configured, the container rejects the TLS connection.
- No authentication frames or JSON-RPC method calls were sent to MEMORY ALPHA. The command closes the WebSocket after upgrading.

Two Caddy root certificates were present in the macOS system keychain. Independent Rust checks rejected the stale root and accepted this one:

`SHA256 9da517d44e10e57108742ff9becaf8d477d1f9f3b745266c35f49e17139a607f`

The public certificate was exported from the trusted keychain, validated against the endpoint, and copied outside Git to `/Users/randydodson/.config/truenas-exporter/caddy-root-ca.pem`. The certificate file is public, user-owned, mode 0644. No CA was added to a trust store and no Caddy/TrueNAS configuration was read or changed through administrative access.

The Rust client uses Mozilla roots plus the explicitly supplied private CA, not macOS Keychain trust. Caddy handles its own upstream TrueNAS certificate verification; that infrastructure configuration is operator-reported and was not modified by this test.

## Historical provisioning gate (superseded below)

The operator selected `prometheus_exporter` and `/Users/randydodson/.config/truenas-exporter/memory-alpha-api-key`, but explicitly states the account/key are not created yet. No placeholder API-key file was created, and no administrator credential was substituted.

- WebSocket transport: PASS on host and container.
- WebSocket authentication: NOT ATTEMPTED; dedicated credentials not provisioned.
- Live `pool.query`: NOT ATTEMPTED.
- Live API methods verified: none yet. Offline tests exercise auth.login_ex and pool.query.
- Appliance version: operator-confirmed 25.04.2.6; not independently queried by this exporter yet.
- Captured API fixtures: none; do not label synthetic fixtures as captured.

## Original operator provisioning sequence

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
