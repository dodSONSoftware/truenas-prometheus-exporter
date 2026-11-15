# Bounded live exporter shutdown/restart acceptance

Prepared November 14, 2026, release 0.8.7. **Proposed procedure; not executed.** This preparation does not authorize a production stop. Review and authorize one execution window before running the commands below. The exporter resumes its existing, previously authorized read-only NAS collection after restart.

## Target and scope

| Item | Required value or boundary |
| --- | --- |
| Docker host | worker@10.10.10.60, observability-stack; not the NAS |
| Container | truenas-exporter, Compose project observability, service truenas-exporter |
| Network and endpoint | observability, internal HTTP port 9110; no published host port |
| Deployed image | local/truenas-rust-exporter:0.8.0 |
| Recorded image ID | sha256:c8f69fc12a105a740e473e7b31a08d00ad06a5fa099263ac446c15efe79551b4 |
| Prometheus | existing container prometheus, host API http://127.0.0.1:9090, job truenas-api-exporter, instance truenas-exporter:9110 |
| NAS target | memory-alpha, existing verified-WSS identity and mounts; no new API client |
| Collection | pool/system polls 60s, API timeout 10s, pool/system TTL 180s; realtime interval 2s and default TTL 6s |
| Dashboard datasource | afpqtre5ghmgwe; dashboards dbfqrjypqhwirke and truenas-memory-alpha-monitor |

Repository version 0.8.7 is not the deployed executable version. Preflight must verify the recorded image ID and actual settings rather than assume a newer local review image is deployed. Production source matches the 0.8.0 source in the inspected repository history; later startup-race changes are test-only.

The only planned service mutations are one graceful stop and one start of this exact container ID. At most two additional idempotent start commands are reserved for restoration: one independent guard and one operator fallback, each only if needed. No container recreation, image pull/load/upgrade, Compose up/down, Docker restart policy edit, volume operation or configuration reload is included. A manually stopped unless-stopped container needs an explicit start; the restart policy is not the restoration mechanism.

Do not stop/restart the NAS, middleware, Netdata, networking, storage or any NAS workload. Do not alter keys, roles, trust, collectors, subscriptions or collection intervals. Do not open a NAS shell, trigger a scan, block traffic or induce a NAS fault. Do not run extra WSS preflights/chart calls. Existing daemon allowlists and normal reconnect behavior remain unchanged. The window bounds observation and control, not the lifetime RPC count of the continuing daemon.

Expected impact is a temporary gap in Rust-exporter telemetry and dashboard availability indicators. It does not represent a NAS hardware outage. Existing automations may observe missing telemetry during the window; choose an operator-approved time without changing or suppressing those automations.

## Time, request and evidence bounds

One window has a **480-second wall-clock limit**, measured with a monotonic controller clock. Finish normal work within 450 seconds, leaving 30 seconds for restoration and final state reporting. No automatic rerun of the window is allowed.

| Phase | Maximum time | Required outcome |
| --- | ---: | --- |
| Preflight | 20s | Correct container/image/settings; no drift; healthy baseline prerequisites |
| Baseline | 60s | Fresh direct measurements and successful normal Prometheus scrapes |
| Arm restoration guard and graceful stop | 25s (10s Docker stop grace) | Exact container exits normally without forced termination |
| Stopped observation | 70s after stop completes | Container stays stopped and Prometheus records a fresh up=0 sample |
| Planned start command | 15s (10s command deadline) | Same container starts with same image and mounts |
| Recovery | 150s after start completes | All six collectors and normal Prometheus scraping recover |
| Stable recovery | 80s | All groups stay fresh and scheduled pool/system snapshots advance again |
| Final checks | 20s | Dashboards recover, other services/configuration unchanged, guard accounted for |

Phase maxima total approximately 440 seconds; the guard is armed within the stop phase and its independent deadline runs concurrently. Abort normal observation at 450 seconds even if a phase would otherwise allow more time. A phase failure is a failed or inconclusive test, never an excuse to extend the window. Restoration commands have their own explicit command deadlines inside the final reserve. Docker-host failure can prevent restoration; report that condition instead of claiming cleanup succeeded.

Observe direct /healthz, /readyz and /metrics at most once each per five-second tick, skipping missed ticks rather than overlapping requests. Each request has a two-second total deadline; cap a metrics body at 256 KiB and health bodies at 4 KiB. Overall direct HTTP cap is 288 requests. Use the selected container network IP from inspect; resolve it again after start rather than assuming it is stable. These HTTP requests read cached exporter state and do not trigger NAS calls.

Prometheus observations are at most one up query and one timestamp(up) query per ten-second tick (48 of each within the global window). Add at most seven distinct dashboard expressions at baseline and seven after recovery, plus one target-inventory read before/after: **112 Prometheus HTTP requests maximum**, each with a three-second deadline and 256 KiB response cap. Keep normal scraping unchanged at 30s with the existing 10s scrape timeout. A bound breach fails the observation; do not fetch a larger payload or increase polling.

Persist only timestamps, durations, selected container/image/state fields, metric names/selected labels/finite values, exit/signal categories and pass/fail reasons. Never save raw Docker inspect output, environment dumps, credential contents, WSS payloads or arbitrary logs. Record UTC timestamps plus the operator's America/Chicago date. Use a private mode-0700 task directory on the Linux host; retain sanitized evidence only after review. Do not read the secret or CA contents. Inspect only the necessary mount metadata and explicitly selected non-secret settings.

## Preflight and baseline gates

Perform these checks read-only on the Docker host before arming the guard or stopping anything:

1. Resolve the container by name and capture its full 64-character hex ID. Pin every later mutation to that ID. Require running/healthy, OOMKilled=false, and a stable restart count. Check Compose project/service labels, exact image ID, user10001:10001, read-only root, dropped capabilities, security option, resource limits, init, restart policy and network against deployment/observability-stack-exporter.json. Verify stop signal is unset/default SIGTERM or explicitly SIGTERM; abort for another signal. Verify the original container and image still exist immediately before stopping.
2. Read the actual whitelisted poll/timeout/TTL/feature/listen settings in memory and persist only those values. An absent realtime TTL means interval times three, not the pool TTL. Abort before stop on configuration drift or unsupported command flags. Do not dump .Config.Env or read .env.
3. Hash only docker-compose.yml and prometheus-config.yml in /mnt/observability-stack. Save the IDs, start times and health of prometheus, grafana, grafana-mcp and loki. No sudo or file edits are needed for the test itself. Confirm the Graphite job/container are absent.
4. Require healthz200 and readyz200, truenas_api_connected=1, truenas_api_compatible=1, truenas_realtime_subscribed=1, truenas_exporter_mock_mode=0 and exactly one successful/fresh operational row for each of pools/system/cpu/memory/network/arc, with the expected target and no duplicate collector evidence. Record their last-success timestamps, snapshot ages and selected measurements. Direct snapshot ages must be below their configured TTLs; all six last-success timestamps must exist and be finite.
5. Require one configured Rust Prometheus target, up=1 and a recent scrape sample. Record the complete target health inventory for comparison; pre-existing down targets are not test failures. The last recorded baseline had 21 targets and three down (Metric Simulator and the two Windows exporters), but use the actual preflight snapshot rather than hard-code those counts as current fact.
6. Re-fetch both saved dashboards and execute their seven distinct NAS expressions through the production datasource or corresponding Prometheus API. Require all twelve NAS panels' underlying expressions to yield finite samples with intended labels. Leave MEMORY BETA unconfigured. Verify no old Graphite metric/job references. Save query hashes so later evaluation uses the same expressions without changing dashboards.

If any prerequisite fails, **do not stop the exporter**. Record a baseline failure and end the window. Existing Linux backups remain rollback references; a simple stop/start requires no configuration restore or new backup of credentials.

## Independent restoration guard

Before issuing stop, arm a separate Linux-host process that survives loss of the SSH/control session. It must target the captured container ID, wait **120 seconds from arming**, then inspect that exact ID and start it once if it is not running. Inspect has a five-second deadline, start a ten-second deadline, and the guard has a 150-second hard lifetime (148s timeout plus a 2s kill allowance). Save its PID and a small completion/status record in the private task directory. A guard that cannot be armed within five seconds or whose ID cannot be verified is a pre-stop abort condition. All timeout durations include up to the stated additional two seconds when --kill-after is used; the phase and global ceilings still take precedence.

The following is the bounded command shape, run by worker after CID and TASK_DIR have been validated. It is a preparation example, not a command executed for this milestone. The controller must confirm the ready marker before proceeding. TASK_DIR must be a fresh private directory owned by worker, and CID must match the container resolved in preflight.

```sh
# Executed on 10.10.10.60 only. TASK_DIR and CID come from preflight.
case "$CID" in ''|*[!0-9a-f]*) exit 1 ;; esac
[ "${#CID}" -eq 64 ] || exit 1
nohup timeout --signal=TERM --kill-after=2s 148s sh -c '
  task_dir=$1
  container_id=$2
  printf "%s\n" armed > "$task_dir/guard.ready"
  sleep 120
  state=$(timeout --kill-after=2s 5s docker inspect --format "{{.State.Running}}" "$container_id") || state=unknown
  if [ "$state" = true ]; then
    printf "%s\n" already-running > "$task_dir/guard.result"
  elif timeout --kill-after=2s 10s docker start "$container_id" > "$task_dir/guard.start-output" 2>&1; then
    printf "%s\n" restore-start-succeeded > "$task_dir/guard.result"
  else
    printf "%s\n" restore-start-failed > "$task_dir/guard.result"
    exit 1
  fi
' guard "$TASK_DIR" "$CID" > "$TASK_DIR/guard.log" 2>&1 < /dev/null &
GUARD_PID=$!
```

This guard does not create or restart a running container. Its start is emergency restoration, **not a passing planned recovery**. If it fires, mark the planned test failed even if telemetry subsequently returns. Confirm its completion or account for the known deadline before leaving the host; never kill the only restoration guard while the container is stopped. If normal control aborts, the operator may issue one additional ten-second docker start on that same ID as the reserved fallback. Log both guard and fallback activity; never loop start/restart, create replacements or silently retry the test.

## Stop, observe and restart

Capture the stop monotonic/UTC start time. Execute only:

```sh
timeout --signal=TERM --kill-after=2s 15s docker stop --timeout 10 "$CID"
```

The ten-second Docker grace sends the verified SIGTERM first and can force termination at expiry. Force is limited to this exporter container, never the NAS, and is an acceptance failure. Require successful command completion, stopped state and exit code0, no OOM, and no intervening auto-restart. Record stop duration and FinishedAt. Read only bounded, sanitized lifecycle evidence for this ID to detect SIGKILL; do not infer a graceful stop merely from eventual absence. Do not start a new observer API session to the NAS.

For shutdown signal evidence, bound one historical event query to the recorded stop interval (with a three-second command deadline and at most 64 records / 16 KiB). Persist only selected signal/exit fields, for example:

```sh
timeout --kill-after=2s 3s docker events \
  --since "$STOP_BEGIN_UTC" --until "$STOP_END_UTC" \
  --filter type=container --filter container="$CID" \
  --filter event=kill --filter event=die --filter event=stop \
  --format '{{.TimeNano}} {{.Action}} {{index .Actor.Attributes "signal"}} {{index .Actor.Attributes "exitCode"}}'
```

The two UTC interval boundaries must be fixed and in the past so this query cannot become an unbounded event subscription. Missing/unavailable lifecycle evidence makes graceful-exit acceptance inconclusive. Do not save all event attributes.

During the next 70 seconds, require a fresh Prometheus up=0 sample for the unchanged Rust target. Use `timestamp(up{job="truenas-api-exporter",instance="truenas-exporter:9110"})` to establish sample freshness: the normal instant-query value timestamp alone is evaluation time, not scrape receipt time. Require that sample timestamp to be after stop completion. A missing target/up series is unknown and fails this criterion, rather than counting as observed down. Direct exporter HTTP should be unavailable once stopped; never require healthz503 from a process that no longer exists.

Dashboard gauges may become No data or briefly retain their last sample according to staleness/panel behavior. Do not expect fabricated zeros. The MEMORY ALPHA status expression must cease reporting green; 0 or -1/unknown is acceptable for loss of exporter evidence. Neither value proves NAS hardware down. Observe this from the saved expression/underlying evidence without editing the tile or sending an n8n report.

After the stopped observation, start the same container once:

```sh
timeout --signal=TERM --kill-after=2s 10s docker start "$CID"
```

Record the new StartedAt, start completion time and current network IP. The ID/image/settings/mounts must be unchanged. Do not use docker restart, compose up or an image replacement. Do not reset metrics or collection intervals to accelerate recovery.

## Recovery and stable-operation pass criteria

Allow at most 150 seconds from start completion for all criteria below. The existing immediate poll can precede API authentication and fail unavailable; the following 60-second scheduled tick normally supplies pool/system data. Temporary readiness503 or nonzero startup error counters are therefore permitted, but every final group must recover. Counter resets across a process restart are expected.

- healthz200, readyz200, mock_mode0, api_connected1, api_compatible1 and realtime_subscribed1, plus healthy container/no OOM and no unexpected additional restart.
- Exactly six successful/fresh collector rows, with direct ages below the configured group TTL. Each group's finite last-success timestamp must be **after the new StartedAt** and newer than the baseline. Require host/controller wall clocks to agree within two seconds in preflight; use monotonic durations and record uncertainty. Abort before stop on a larger offset, and reject last-success timestamps beyond that measured uncertainty.
- Prometheus up1 with an actual scrape sample after new StartedAt and an increment beyond its stopped-phase sample timestamp. Normal scraping must be sufficient; no manual reload, scrape-interval change or special scrape trigger is allowed.
- Actual selected pool/system/CPU/memory/network/ARC measurements must be present with their baseline identities and valid units. A stopped-phase cache value or an up1 scrape alone does not prove NAS collection recovered. Legitimate measurement changes during normal NAS activity are allowed.

Maintain those conditions for the next 80 seconds using the same bounded observations. Pool/system last-success timestamps must advance again during that stable interval; realtime groups must also advance. One failed readiness/freshness observation breaks this acceptance interval and fails the window; do not restart the interval to obtain a pass. This is not a longer soak or a guarantee about all future outages.

At final verification, execute the seven saved dashboard expressions again, require twelve NAS panels' data to recover and the status expression to return 1, and compare the other service/container identities, target inventory and configuration hashes against baseline. Unexpected external environment changes make attribution inconclusive; record them rather than claiming the test caused or fixed them. Recheck exporter running/healthy and account for the restoration guard. Leave the exporter running.

## Failure handling and result record

On any failure after stop, skip remaining acceptance observations and restore the existing container using the bounded guard/operator allowance. Verify running and health/readiness if time remains; do not claim readiness when only running state was recovered. If the existing container or Docker host cannot be restored inside the remaining budget, report exact observed state and operator action needed. Do not restore Graphite, recreate a deleted NAS reporting entry, change credentials or modify other services as an automatic response.

A PASS requires every mandatory preflight, graceful-exit, down-scrape, restart, fresh-collection, stable-operation and final-dashboard gate with no guard-assisted restart. Record per-phase durations, signal/exit evidence, all six before/after/final success times, snapshot ages, actual Prometheus scrape times, query results, configuration hashes, image/container IDs and guard disposition. Reject ambiguous/duplicate evidence. A deadline, command failure or missing evidence yields FAIL or INCONCLUSIVE with its reason; no silent rerun.

Successful bounded shutdown plus no cleanup-warning log is useful evidence, but does not independently capture the NAS unsubscribe acknowledgement or exact server-side socket count. Those protocol behaviors already have synthetic coverage. Preserve that distinction. This test also does not close independent numerical/ARC algorithms, minimum-role/write-denial, hosted CI, long-duration resource, stalled-reader-on-live-NAS or induced network-failure acceptance.

## Preparation and authorization status

This milestone prepares the reviewable procedure only. No live inspection, stop/start, guard process or NAS call was performed. The documented guard/controller procedure has not been rehearsed as an executable harness; before execution, perform a local synthetic rehearsal of its deadlines, interruption and restoration paths, and review the exact resulting controller/commands. This is an execution prerequisite, not a claim that a tested live harness already exists.

The separate execution authorization should identify this plan/version, one window of at most eight minutes, the exact Linux container/image, one stop and planned start, the two bounded restoration allowances, read-only observation caps, the temporary telemetry gap, and unchanged NAS services/credentials/collection. Approval to prepare and commit this plan does not imply approval to run it.
