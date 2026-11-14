# Configuration

No secret values belong in environment files, logs or Git. Config reads env once. Required in normal mode: TRUENAS_WS_URL, TRUENAS_USERNAME, TRUENAS_API_KEY_FILE. No hardcoded NAS address. Username must be nonempty. Key file must be UTF-8, nonempty, <=16 KiB; trailing line endings are removed. Reject control characters in key/username. Optional TRUENAS_CA_CERT_FILE must contain valid trusted PEM certificates and be at most 1 MiB. Target, key and CA inputs must resolve to regular files. Symlinks to regular files are supported. Reads use one opened handle and consume at most the byte cap plus one byte before rejecting excess content. Limits apply before parsing or trimming line endings. Secret-bearing structs intentionally have no Debug implementation.

| Setting | Default | Validation |
| --- | --- | --- |
| EXPORTER_LISTEN | 0.0.0.0:9110 | Numeric socket address |
| EXPORTER_MOCK | false | true/false; true bypasses all API config and makes no network connections |
| TRUENAS_POLL_INTERVAL_SECONDS | 60 | Integer 1..86400 |
| TRUENAS_API_TIMEOUT_SECONDS | 10 | Integer 1..300 |
| TRUENAS_CACHE_TTL_SECONDS | 3 x poll interval | Greater than poll interval + API timeout |
| RUST_LOG | info | tracing-subscriber filter |

TRUENAS_HOST and TRUENAS_REALTIME_INTERVAL_SECONDS from the handoff are reserved for M3 and not consumed by M2. Do not claim realtime monitoring in this milestone.

Normal mode accepts only wss URLs without embedded credentials, query or fragment. There is no insecure runtime flag. Mozilla trust roots plus optional private CA are used. A matching certificate DNS name or IP SAN is required. Unreadable/invalid local files fail startup; remote outages keep HTTP alive with readiness 503 after cache expiration.

Healthcheck: `truenas-exporter healthcheck` sends a bounded local HTTP request to /healthz (wildcard listen addresses are translated to loopback). It does not load credentials or contact the NAS. SIGINT/SIGTERM cancel background tasks and gracefully stop HTTP.

Only exporter-owned redacted tracing events are emitted. Dependency logs are filtered even when RUST_LOG=trace so WebSocket frame traces cannot expose authentication credentials.

## Multiple targets

Set `TRUENAS_TARGETS_FILE` to a JSON file containing a targets array. Each entry requires name, ws_url, username and api_key_file; ca_cert_file is optional. `deployment/targets.example.json` contains the MEMORY ALPHA container paths. Only references to secret files are allowed; unknown fields (including inline api_key) are rejected.

- Configure 1..16 targets. Names must be unique, 1..64 ASCII letters/digits/dots/underscores/hyphens.
- Key/CA paths in this file must be absolute and refer to paths visible inside the exporter process/container. The file is at most 64 KiB and read at startup; edit it and restart explicitly to add a server.
- File mode cannot be combined with EXPORTER_MOCK=true or legacy TRUENAS_TARGET/WS_URL/USERNAME/API_KEY_FILE/CA_CERT_FILE settings. Ambiguous configuration fails startup.
- Poll interval, API deadline and TTL settings remain shared defaults. Each target has its own connection, credentials, CA store, request map, backoff, collector, errors and cache.
- All measurements and operational series have target="configured-name". A failed or expired target does not remove another target's data. /readyz requires all configured targets to have fresh required snapshots; /metrics and /healthz remain available during partial outages.
- A single registry emits one HELP/TYPE descriptor per metric and one EOF marker across targets.

Legacy single-target environment configuration remains supported. Without TRUENAS_TARGET it preserves the original label sets; setting TRUENAS_TARGET=memory-alpha opts into the same target label used by file mode. `deployment/.env.memory-alpha.example` documents the proposed native settings; it does not create credentials.

## Transport diagnostic

`truenas-exporter check-transport` reads only TRUENAS_WS_URL, optional TRUENAS_CA_CERT_FILE and the API timeout. It performs verified WSS upgrade and close without loading an API-key file, authenticating, or calling any API method. Certificate and hostname checks cannot be disabled. It intentionally does not load TRUENAS_TARGETS_FILE; select one URL/CA explicitly for this diagnostic. The integration record is in memory-alpha-integration.md.
