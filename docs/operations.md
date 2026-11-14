# Operations and deployment boundaries

Development uses synthetic fixtures and loopback servers only. MEMORY ALPHA (10.10.10.7) is strictly off-limits until an operator explicitly authorizes a specific interaction. No test is allowed to use production credentials. Automated tests remain offline. An explicit operator-authorized transport-only command is available; see memory-alpha-integration.md. The operator has provisioned dedicated credentials and completed the specifically recorded checks; any broader live action still requires matching authorization.

Local: `docker compose -f deployment/compose.yaml config` then `docker compose -f deployment/compose.yaml up --build`. This explicit mock service exposes 9110 only inside its Docker network. Use docker compose exec with the built-in healthcheck for liveness; attach a local Prometheus service to that network to scrape truenas-exporter:9110. A developer may explicitly add a loopback host port mapping for local inspection. The mock stack is not a competing production stack.

Production examples are opt-in artifacts. Use a built, reviewed image and merge the example fragment into the existing observability project only after authorization. Paths resolve from that Compose project's directory, not from the Mac checkout. Readable secret/CA mounts must match UID/GID 10001; root:root 0600 files will not work. No Docker socket, privileged mode or TrueNAS installation. Certificate trust and credential setup must be performed by the operator through supported TrueNAS administration, after the role matrix is verified; do not send secrets through chat.

Run `docker compose config` on the eventual merged configuration before rollout. Verify its actual network name and service DNS. Append the sample scrape job to existing scrape_configs; never replace the existing file. No 9108/9109 port reuse, no Graphite removal or automatic dashboard import. Both jobs must remain during a separately authorized comparison.

Container is non-root, read-only, all capabilities dropped, no-new-privileges, with /healthz healthcheck. /readyz is not a restart trigger. Finite synthetic resource profiles are recorded in isolated-cgroup-validation.md; their sampled values do not establish universal production bounds. TLS failures emit a safe category and trust/name guidance; keys, URLs, frames and remote errors are not logged. Repeated failures use bounded retries.

License: MIT. See the root LICENSE. Local linux/amd64 runtime checks pass through TN-15. RustSec and image scan results are historical until refreshed; medium/low findings remain tracked. Captured pool/system/realtime fixtures and scoped live checks now exist; see security-validation.md and validation.md.

The runtime has no shell or package manager. Use logs/metrics and `truenas-exporter healthcheck` for diagnosis; do not add tools to the deployed image. The runtime validation probe is exported and mounted only during local/CI tests. OpenSSL/Python are host-side test requirements, not runtime dependencies.

The integration fragment now consumes a read-only target JSON file plus distinct key and public Caddy CA mounts. Set TRUENAS_TARGETS_SOURCE, MEMORY_ALPHA_API_KEY_SOURCE and CADDY_CA_SOURCE on the Docker host; deployment/.env.example contains path examples only. Credentials are mounted at /run/secrets/memory-alpha-api-key and the public CA at /run/secrets/memory-alpha-caddy-root-ca.pem. Add each future server's entry and secret mounts explicitly. No host trust store or runtime image mutation is required.

The example fragment does not create a production network or account, and was not applied. Use paths appropriate to the eventual Docker host; the Mac's ~/.config path is not assumed to exist on the observability VM.


Container validation reports success after removing its temporary containers. A cleanup failure identifies affected containers and makes an otherwise successful run fail. If validation also fails, its original exception is retained and cleanup errors are written to stderr. Failed starts that leave no container are distinguished with a successful Docker list query. The script attempts cleanup of every name registered by its run.


## Integrated offline operational validation

The opt-in sustained system/realtime test and its container reproduction are documented in [integrated-operational-validation.md](integrated-operational-validation.md). It uses generated synthetic credentials and loopback TLS only, with two sockets per target, silence/outage/recovery and stalled-reader shutdown. Four sustained tests are intentionally ignored by ordinary cargo test. Select exact --ignored test names for local-child cases; launch the isolated case through its controller. Child CPU/RSS and whole-cgroup resources have different scopes; neither a finite pass nor a capped run certifies long-term memory stability or production readiness.

The [phased memory profile](operational-memory-profile.md) adds integrated_memory_profile, including 30 seconds of warmed baseline, two minutes of scrape pressure, 45 seconds of cooldown and three recovery cycles under a 900-second deadline. It uses the same synthetic fixture; no appliance authorization or production key is needed.


[Separate-cgroup validation](isolated-cgroup-validation.md) uses scripts/profile-isolated-cgroups.py to run the fixture and exporter in separate containers/cgroups sharing only the network-disabled loopback namespace. It requires an explicit local Docker context and rejects real endpoints/credentials. Invoke this fourth sustained scenario through that controller; select exact test names for the three in-process scenarios instead of running every ignored operational test indiscriminately.

The executable --version reports the root VERSION release. For the observability Compose example, set TRUENAS_EXPORTER_IMAGE to the reviewed immutable image digest or release tag. The local synthetic Compose build uses the dev tag.

## Local Mac observability stack — November 14, 2026

The operator authorized ongoing read-only exporter collection in a separate container in the existing metric-simulator-observability Docker Desktop project. Prometheus and Grafana remain in that project. Use Docker context desktop-linux. Grafana was upgraded with approval to 13.2.2, commit 1bea008f7e4e858b6824c9e364d608bd4d10b13a.

The base Compose file is /Users/randydodson/Documents/_work/prometheus-metrics-simulator-docker-volumes/compose.yaml. Merge it with this repository's deployment/compose.local-dashboard.yaml using project name metric-simulator-observability. The local base project's .env supplies TRUENAS_EXPORTER_IMAGE and the public TRUENAS_CA_CERT_SOURCE path. It also retains existing local UID/GID settings. These paths belong to the Mac setup.

```sh
docker --context desktop-linux compose -p metric-simulator-observability \
  -f /Users/randydodson/Documents/_work/prometheus-metrics-simulator-docker-volumes/compose.yaml \
  -f /Users/randydodson/Documents/__development/truenas_metrics/deployment/compose.local-dashboard.yaml \
  up -d
```

The exporter uses verified WSS, the existing dedicated identity, 60-second pool/system polls and a two-second realtime subscription. Its runtime is non-root UID/GID 10001, read-only, capability-free, no-new-privileges and limited to 128 MiB. A dedicated external volume truenas-local-exporter-credential holds only the mounted key, owned by 10001:10001 with mode 0400. The original operator credential remains separate. Rotation must update the protected copy and restart only the exporter; never print its contents. The CA is a read-only bind mount. Loopback ports are Grafana 3000, Prometheus 9090 and exporter 9110. These are local-development services, separate from the Linux production stack.

Prometheus job truenas-api-exporter scrapes truenas-exporter:9110 every five seconds. This scrapes the cache; NAS collection intervals are unchanged. The previous simulator scrape job is preserved. Its configuration was backed up before adding the exporter job and validated using promtool before reload.

The dashboard UID is truenas-rust-exporter, titled TrueNAS · API Exporter in folder TrueNAS. Source JSON is dashboards/truenas-exporter.json. The local Grafana provider is config/grafana/provisioning/dashboards/truenas.yaml under the base project; its dashboard copy is dashboards/truenas/truenas-exporter.json relative to that provisioning directory. Built-in panels cover all 34 cataloged metric families and Prometheus up. Filters select datasource, scrape job, instance, TrueNAS target, pool, interface and CPU. Missing/expired values remain absent; no error counter is fabricated before its series exists. ARC ratios retain explicit source-semantic caveats.

Grafana 13.2.2 successfully persisted all 40 panel/row entries and seven variables through file provisioning. All panel queries parsed and ran against local Prometheus. The connection-error family had no series before a polling connection error. No browser visual inspection was available in this run.

Before the Grafana upgrade, the stopped service's data, provisioning and Compose file were archived at /Users/randydodson/Documents/_work/prometheus-metrics-simulator-docker-volumes/backups/grafana-before-13.2.2-20261114T232518Z.tar.gz, mode 0600. Restoring 12.1.0 requires stopping Grafana and restoring this data backup as well as the earlier image configuration. Do not downgrade a migrated database in place. The Prometheus scrape backup is alongside its configuration. For exporter rollback, stop only truenas-exporter or select a previously reviewed image; preserve monitoring data and the protected credential volume. Use stop rather than down -v for routine shutdown.
