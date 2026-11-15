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

## MEMORY ALPHA overview — 0.7.0

The top row pins target memory-alpha and respects scrape job/instance filters. It displays configured IP 10.10.10.7 above cached uptime, aggregate CPU, 1-available/total memory, allocated/size per selected pool, and RX+TX across selected interfaces. The system tile reflects Prometheus scrape availability, API connectivity and fresh successful collectors. Pool/interface filters apply to the corresponding overview panels. Select physical interfaces to avoid counting overlapping paths. All 40 original detailed panel/row entries retain their IDs and move down seven grid units. The dashboard now has 47 panel/row entries and seven variables.

## Supplied office-desk dashboard — 0.8.0

The edited resource is dashboards/office-desk-memory-alpha.json, retaining the supplied Grafana v2 metadata name dbfqrjypqhwirke. New panels 112–115 occupy x=8/10/12/14, y=10, width=2 and height=5 beside MEMORY ALPHA. The original empty panel-100 layout area is shortened to y=15, height=5, leaving its lower backup-row placeholder. All original element definitions and other layout entries are preserved.

At release 0.8.0 the four additions used local Prometheus UID prometheus. The later live MCP integration uses the production datasource recorded below. Both versions use exact exporter job/instance/target labels. Existing panels retain their original datasource references. The network gauge selects enp42s0, verified in local Prometheus at 1 Gbit/s. Its 250 MB/s scale represents combined full-duplex RX+TX capacity; revise the scale if link speed changes. All four new query expressions returned live samples. At release 0.8.0 this resource was a file deliverable. The later explicitly authorized MCP update is recorded below. Import it in the intended Grafana instance or use its resource JSON in the editor; ensure datasource UID prometheus supplies the selected exporter series there.

## Authorized Linux installation — November 14, 2026

The operator explicitly authorized installing the exporter on worker@10.10.10.60 in /mnt/observability-stack and separately approved transferring the existing dedicated NAS API credential using encrypted SCP. Docker access is available to worker; sudo is required for /mnt changes. No SSH password or API key value was saved in this repository.

An exporter-only snapshot of the installed Compose service/network is saved in deployment/observability-stack-exporter.json. It includes file paths, never key contents.

The installed service truenas-exporter belongs to Compose project observability, in the existing docker-compose.yml, on network observability. It runs local/truenas-rust-exporter:0.8.0 (linux/amd64), transferred as a local image archive and loaded on the server. The executable reported 0.8.0. Release 0.8.1 records deployment evidence and configuration. No image registry publication is involved.

The service uses UID/GID 10001:10001, read-only root, zero capabilities, no-new-privileges, 128 MiB memory limit, 0.5 CPU, 64 PIDs and bounded logs. It restarts unless stopped. Port 9110 is exposed only on the Docker network. No host/LAN port is published. Prometheus reaches http://truenas-exporter:9110/metrics on the shared network.

The dedicated key is installed at /mnt/observability-stack/secrets/truenas-exporter/api-key, owner 10001:10001 and mode 0400. The public CA is root-ca.pem in that root-only directory, mode 0644. Both files are mounted read-only. The temporary worker-owned key copy was removed after installation. Future rotation must update this Linux copy through an approved secure transfer and restart only this service. The source Mac credential was not modified.

Collection uses wss://memory-alpha.home.lab/api/current, username prometheus_exporter, target memory-alpha, 60-second pools/system polling, two-second realtime interval and 180-second pool/system TTL. Existing API allowlists and verified TLS remain in force. Startup unavailable attempts before admission cleared after the first scheduled poll. Linux readyz returned 200 and all six collectors became successful.

The existing Prometheus file prometheus-config.yml adds job truenas-api-exporter at 30 seconds, target truenas-exporter:9110, environment Hardware, ipaddress 10.10.10.7 and moniker TrueNAS API. The distinct moniker preserves the legacy up{moniker="TrueNAS"} selector for the independent Graphite target. All existing jobs were preserved. Candidate Compose validation and promtool checks passed before applying changes. Prometheus reload used POST http://127.0.0.1:9090/-/reload. The bind-mounted configuration was updated in place so the running container sees the same inode.

Backups of docker-compose.yml and prometheus-config.yml are in /mnt/observability-stack/backups/truenas-exporter-20261115T012526Z. This UTC timestamp falls on November 14 in the operator's America/Chicago timezone. The backup directory is root-only. Rollback requires sudo: stop only truenas-exporter, restore those two configuration contents in place, validate Compose and Prometheus configuration, and reload Prometheus. Preserve Grafana/Prometheus data. Keep the dedicated credential protected until the operator decides whether the service will be resumed. Avoid down -v on the observability project.

Useful server checks follow. Metrics are read over the container network; no new NAS action is required for these checks.

```sh
cd /mnt/observability-stack
docker compose -p observability -f docker-compose.yml ps truenas-exporter
docker logs --tail 20 truenas-exporter
docker exec prometheus promtool check config /etc/prometheus/prometheus.yml
curl -fsSG http://127.0.0.1:9090/api/v1/query --data-urlencode 'query=up{job="truenas-api-exporter"}'
curl -fsSG http://127.0.0.1:9090/api/v1/query --data-urlencode 'query=truenas_exporter_collection_success{job="truenas-api-exporter"}'
```

Minimum effective roles, independent raw-counter/ARC algorithm verification, formal induced-fault/recovery acceptance and longer resource confidence remain separate findings. Successful installation and current health do not close those evidence gaps.

## Live office-desk MCP update — 0.8.2

The operator explicitly authorized using Grafana MCP to add CPU, MEMORY, POOL USED and NETWORK RX + TX controls directly to the MEMORY ALPHA row on office-desk. Dashboard UID dbfqrjypqhwirke in folder rtx4g9 was updated from version160 to161. Saved version160 remains the rollback snapshot. The version-history message is Add MEMORY ALPHA CPU, memory, pool used and RX+TX gauges from the Linux Rust exporter.

The targeted patch added elements panel-112 through panel-115 and four grid items at x8/10/12/14, y10, width2 and height5. It moved the empty panel-100 placeholder to y15, height5. Every existing element and unrelated layout/dashboard property was verified unchanged against the fetched live source. The repository export dashboards/office-desk-memory-alpha.json now reflects the saved live resource.

All four new panels use production Prometheus UID afpqtre5ghmgwe, job truenas-api-exporter, instance truenas-exporter:9110 and target memory-alpha. CPU is aggregate percent; memory is 100*(1-available/total); pool allocation is per-pool percent; enp42s0 RX/TX rate gauges are added directly in bytes/s. The network gauge scale is 250 MB/s for the reported full-duplex 1 Gbit/s link. Adjacent gauge styles were reused, with stale field-name filters cleared on the network panel. All four queries returned production samples through MCP before saving, and a subsequent dashboard fetch confirmed the new definitions.

The optional Grafana panel image-render request returned HTTP500. Saved-model/query verification passed; screenshot rendering was unavailable. No renderer configuration was changed.
