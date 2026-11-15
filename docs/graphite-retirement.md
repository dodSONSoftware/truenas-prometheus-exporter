# Graphite retirement — November 14, 2026

The operator authorized migrating office-desk and TrueNAS Server Monitoring, checking remaining dependencies, then stopping Graphite and verifying the replacement before removal. A separately approved bounded change migrated the active n8n Daily Infrastructure Report. No NAS configuration, credentials, roles, scans or administrative APIs were changed.

## Dashboard changes

- office-desk (`dbfqrjypqhwirke`, saved version 163): MEMORY ALPHA status and uptime now use the Rust exporter. The existing CPU, memory, pool-used and RX+TX gauges already used it. MEMORY BETA remains a gray, unconfigured placeholder with no queries. Only elements 30, 85, 110 and 111 changed during the live patch; layout and all other live elements were preserved, including the operator's blank gauge titles.
- TrueNAS Server Monitoring (`truenas-memory-alpha-monitor`, saved version 6): status, uptime, aggregate CPU, physical-memory utilization, ARC size and network RX+TX now use the Rust exporter. Network rates are bytes/second gauges, with a 250 MB/s combined full-duplex ceiling for enp42s0's reported 1 Gbit/s link. No rate() or Graphite scaling is applied. Layout is unchanged.
- Both use Prometheus datasource UID `afpqtre5ghmgwe`, job `truenas-api-exporter`, instance `truenas-exporter:9110`, target `memory-alpha`.
- Green means a successful exporter scrape and authenticated NAS API connection. Red means explicit connection/scrape failure. Missing evidence is unknown. This is an API availability indicator, not proof of hardware power state or complete pool health. The fallback `or on() vector(-1)` produces a single unknown result only when the left side is absent.

Canonical v2 resources are saved in dashboards/office-desk-memory-alpha.json and dashboards/truenas-server-monitoring.json. An earlier image-render attempt returned HTTP 500; this milestone verifies saved resources and live queries, not screenshots.

## Dependency investigation and report migration

All 30 saved Grafana dashboards were inspected, including queries, variables and annotations. No old TrueNAS metric names, Graphite job/9108 endpoint or Graphite datasource UID remained after migration. Some unrelated panels retain historical `group: graphite` serialization alongside PromQL and their existing Prometheus reference; those fields were preserved and are not references to the retired TrueNAS endpoint. There were no Grafana-managed alert rules and no Prometheus rule groups. All three MCP-visible n8n workflows were inspected; the Daily Infrastructure Report was the only additional identified dependency. This is an inventory of visible/configured consumers, not proof that no external client exists.

The report's three changed nodes are Report Configuration, Prometheus Targets and Analyze Targets. Unrelated nodes and connections are unchanged. The query includes exporter up, authenticated API connection, uptime and collection success/age/last-success evidence, with name/target/collector-aware union matching. Exactly one supporting system series is required, within a 180-second snapshot limit and the existing 300-second scrape freshness policy. Missing, duplicate, invalid, stale or disconnected evidence yields unknown, never confirmed host down. Ordinary exporter counts remain based on up only.

The MCP tools could save a draft but had no publish action. The operator published the draft in the UI. Active workflow version `c95b24b0-4a40-49dd-993e-31b89e2d0bee` was re-fetched and verified identical to the tested three-node change. No report was executed or sent. deployment/n8n-truenas-migration.json contains only these non-secret parameter sets before/after and version IDs; it is not a complete workflow or credential export. Twenty-one isolated JavaScript checks pass, including preservation of unrelated-host behavior. A separately fetched live Prometheus response evaluated as healthy API telemetry without executing n8n.

## Linux backup and retirement gate

Host 10.10.10.60 has two separate Compose projects: observability in /mnt/observability-stack and graphite-exporter in /opt/graphite-exporter. The Graphite service has no mounted data volumes. Its sole service uses prom/graphite-exporter:v0.16.0, restart unless-stopped and ports 9108/TCP and 9109/TCP+UDP. No Graphite references were found in the inspected /etc/systemd/system and /etc/cron.d files. Its 621 scraped metric families consisted of TrueNAS-prefixed data and exporter/runtime instrumentation; removal is not a promise that the Rust exporter implements every old Graphite series.

Root-only backup directory:

`/mnt/observability-stack/backups/graphite-retirement-20261115T0209Z`

It contains the pre-retirement prometheus-config.yml and graphite-compose.yml. A candidate Prometheus configuration removes exactly the Graphite job, preserving the other 21 jobs and all global settings. It passed pinned Prometheus v3.15.0 promtool validation before application. Because Prometheus bind-mounts an individual configuration file, apply contents in place to preserve its inode, validate the running mount and use the existing lifecycle reload endpoint.

Graphite was stopped reversibly first. At approximately 98 seconds after the baseline, all six Rust collectors had newer successful snapshots. Readiness returned HTTP 200, container health remained healthy with zero restarts, and Prometheus showed Rust up=1 while Graphite up=0. Each migrated query, including tank usage, returned one finite live sample. The report evidence logic evaluated a newly fetched post-stop response as healthy without sending a report. Only after this gate passed was the Graphite scrape job removed in place, the running mounted file validated and Prometheus reloaded successfully. The 21 remaining targets were verified before removing the stopped container. Its standalone Compose file was archived as /opt/graphite-exporter/docker-compose.yml.retired-20261115T0209Z; the image was retained. Post-removal verification found 21 configured Prometheus targets, Rust scrape up, readyz HTTP 200, healthy container and zero restarts. The Graphite container is absent, ports 9108/9109 are free and the original Compose path is absent with the archive present. Grafana, Prometheus and Loki remained healthy; grafana-mcp remained running. The same three unrelated targets were down before and after: metric-simulator, windows_exporter-10.55 and windows_exporter-10.56. All six migrated monitor query expressions returned finite samples again, and re-fetched dashboard resources matched the saved exports. Prometheus retained the removed job's final instant-query sample temporarily within its normal five-minute lookback. After it aged out, the report query returned exactly 21 up series (18 up, three pre-existing down), no Graphite job, and MEMORY ALPHA healthy/non-degraded under the published evidence logic. No historical data was deleted.

## Rollback and limits

Before container removal: `docker start graphite-exporter` restores the bridge. After removal, restore the backed-up standalone Compose definition to its original path and run that project's service only. Restore Prometheus configuration contents in place from the backup, validate with promtool and reload; do not recreate the whole observability stack. Restore dashboard queries through Grafana version history and the report's three parameters from the before artifact only if reverting the migration too; publish that report rollback separately. Preserve unrelated later configuration changes when using backups.

The operator subsequently authorized disabling NAS-side legacy Graphite reporting and reported clearing Enable and saving the existing entry in the TrueNAS UI on November 14, 2026. No administrative credential or role was changed by Codex. The disabled configuration is retained for rollback. This configuration state is operator-reported; the follow-up checks below independently verify Rust telemetry after the change. Historical Prometheus data remains until normal retention; no TSDB deletion, image prune or volume deletion is needed. An unused Graphite datasource can remain configured without making the Rust dashboards depend on it.

Independent raw numerical verification, ARC source zeros/chart algorithms, formal combined-daemon recovery/minimum-role acceptance, hosted CI and the intermittent process-test root cause remain open. The installed Linux image remains 0.8.0; repository release 0.8.3 records dashboard/operations changes and does not redeploy exporter code. The sampled separate-cgroup 63.30 MiB observation is not a kernel peak or universal guarantee.

## Repository validation

The first full check encountered the known intermittent process failure in malformed_notification_stops_session_without_logging_its_payload (exporter exited prematurely). Its isolated run passed; a subsequent full scripts/check.sh passed formatting, strict Clippy, 83 Rust tests and 18 Python tests. Four sustained cases remained ignored. The 21 isolated n8n tests passed with `node scripts/tests/test_n8n_evidence.cjs`. The pinned linux/amd64 container build passed in Docker Desktop context desktop-linux. No test assertions were relaxed and no long profile was rerun.


## NAS-side disable follow-up — release 0.8.4

Pinned TS-25.04.2.6 source confirms that reporting.exporters.update accepts an enabled-only patch and restarts Netdata. The available dedicated exporter credential remains read-only. The operator used Reporting → Exporters in the NAS UI to disable and save the old Graphite entry; Codex did not perform a NAS write or independently read that administrative configuration.

After the operator confirmed saving, the Linux Rust exporter was healthy with zero restarts, no OOM kill, and HTTP200 from both healthz and readyz. Prometheus scrape up was 1. All six collectors (pools, system, cpu, memory, network, arc) reported success with fresh snapshot ages. Both saved dashboards were re-fetched and contained no legacy TrueNAS Graphite queries. All 12 NAS panels, representing seven distinct expressions, returned one finite live value each: connected status, uptime, CPU, memory, tank allocation, enp42s0 RX+TX and ARC size. MEMORY BETA remained unconfigured. No screenshot/render verification was performed.

The Graphite container and active scrape job remained absent. Prometheus still had 21 targets, with only the same pre-existing Metric Simulator and two Windows exporters down. This is a bounded operational check, not independent numerical accuracy or formal induced-failure/recovery acceptance.

A second observation approximately 108 seconds after the initial follow-up showed strictly newer last-success timestamps for all six collectors, each still successful. System/pool snapshot ages were approximately 15 seconds and realtime group ages approximately two seconds, demonstrating continued collection rather than retained pre-change values. scripts/check.sh passed formatting, strict Clippy, 83 Rust tests and 18 Python tests; four sustained cases remained ignored.

The pinned linux/amd64 image build passed using local Docker Desktop context desktop-linux. No runtime deployment was performed for this documentation release.
