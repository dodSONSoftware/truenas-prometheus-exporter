# Separate exporter and fixture cgroup validation

November 14, 2026. The synthetic phased profile passed with the production exporter and fixture in separate local Docker cgroups. The exporter remained capped at 128 MiB and half a CPU. Its **highest sampled charged memory was 63.30 MiB**, leaving **64.70 MiB of sampled margin**. The prior combined-harness cap hit is not evidence that the exporter alone needs 128 MiB. This is finite workload evidence, not a kernel high-water mark or universal production budget guarantee.

## Isolation and workload

scripts/profile-isolated-cgroups.py runs two UID/GID-10001 containers from the reviewed runtime image:

- The fixture starts with network none, 256 MiB memory with swap disabled and one CPU. It runs the explicitly ignored isolated_cgroup_memory_profile test, generated TLS server, scrape clients and assertions. Its image healthcheck is disabled because it is a test fixture, not an exporter listening at the default port.
- The production exporter has its own PID namespace and cgroup, read-only root, dropped capabilities, no-new-privileges, 16 MiB /tmp tmpfs, 128 MiB memory with swap disabled and half a CPU. It uses the image's actual entrypoint/binary and normal healthcheck. No observer process or Docker socket is mounted in it.
- Only the fixture's network namespace is shared: it is loopback-only and has no external network. No host port, bridge network or appliance endpoint is used. Synthetic configuration/key/CA files are shared at /control; the exporter mount is read-only.
- The host controller rejects non-Unix Docker endpoints, non-loopback fixture URLs/listeners, unexpected settings and non-synthetic key material. Paths must resolve inside its fresh temporary control directory. The profile uses no existing credential file, CA, Prometheus, Grafana or production configuration.

The same cardinalities and phases are retained: 16 targets/32 WSS sessions; 256 pools, 257 CPU entries and 128 interfaces per target; warmed baseline 30 seconds, four-client scrape load 120 seconds, cooldown 45 seconds and three eight-second outages with recovery and quiet periods. Readiness, expiry, active-session recovery, owned shutdown requests and complete server-session drainage are asserted. The exporter is external to the fixture process, so the controller coordinates Docker stop and publishes its exit status back to the test. The test rejects nonzero exporter exit codes.

A generated manifest and marker files coordinate startup/stop. The controller validates the manifest before launching the exporter, waits for the fixture result, and uses the existing failure-preserving/inventory-checked cleanup helper for both owned containers. Final controller code persists a passed report only after cleanup succeeds. Ordinary cargo test still ignores sustained tests; this fourth scenario requires the controller and must not be invoked as a bare standalone test.

## Observed results

The fixture passed in 328.26 seconds, recording 840 complete scrapes, 6100 expected encoder-busy responses, 4384 pool requests, maximum sampled health latency 45 ms and 64 cumulative subscriptions. All three cycles restored exactly 32 active sessions. It verified 16 final owned unsubscribe requests, both containers exited with code zero, and Docker reported exporter OOMKilled=false. Request counts do not independently prove consumption of each unsubscribe acknowledgement; correlation/cleanup deadlines have separate protocol coverage.

| Accounting scope | Highest sampled charged memory | Limit | Margin at highest sample |
| --- | ---: | ---: | ---: |
| Exporter cgroup | 63.30 MiB | 128 MiB | 64.70 MiB |
| Fixture cgroup | 53.42 MiB | 256 MiB | 202.58 MiB |

The fixture independently printed a kernel memory.peak of 54.84 MiB. This demonstrates that sampled maxima can miss short peaks; its difference cannot be applied as an error bound to the exporter. Docker's stats API did not provide the exporter's kernel high-water mark. The exporter peak sample included about 53.51 MiB anon and 6.93 MiB file charges; remaining charges are not fully attributed. Memory.stat categories overlap, and cgroup charges are not equivalent to process RSS or live heap bytes.

| Exporter phase | First / last / maximum sampled charge (MiB) |
| --- | ---: |
| Warmed baseline | 41.16 / 27.67 / 42.94 |
| Scrape load | 35.61 / 63.25 / 63.30 |
| Cooldown | 63.05 / 40.90 / 63.05 |
| Recovered cycle 1 | 34.55 / 35.15 / 35.64 |
| Recovered cycle 2 | 35.42 / 35.53 / 35.70 |
| Recovered cycle 3 | 35.00 / 35.35 / 35.45 |
| Final metrics/stalled-reader setup | 60.58 / 60.58 / 60.58 (one sample) |

The controller collected 323 paired snapshots through read-only Docker stats requests at roughly one-second cadence plus API overhead. Native controller CPU/memory is outside both cgroups. Phase boundaries are observed through the fixture log, so classification has sampling/transport uncertainty. Stop coordination took 2189 ms from the fixture request through controller acknowledgement; this includes controller polling/API overhead and is not an isolated SIGTERM-to-exit measurement.

See [durable evidence](evidence/isolated-cgroup-profile-2026-11-14.json) for phase summaries, selected time series, observed limits, CPU counters, image/harness/source identities and the full raw-report hash. No machine endpoint or credential value is saved. The tested runtime image was sha256:d05fb57e8cdcd300967d8cb5dacec00b0bea8d475907bcf1e83c22809a283669.

## Limits and disposition

This closes the finite separate-cgroup attribution milestone. It establishes successful operation under the exporter cap for this workload, with observed sampled margin. It does not establish the true exporter peak, cgroup memory-event history, indefinite stability, behavior under different payload sizes/cardinalities/scrape policies, or an appliance deployment budget. Shared file-page charges can depend on first touch. The fixture now has an independent CPU/memory allowance, so throughput and combined peaks are not directly comparable to earlier shared-cgroup runs. No production optimization or resource-limit change is claimed.

The earlier native scrape-time RSS peak remains an observed platform-specific limitation; this Linux accounting does not diagnose it. Independent live numerical/chart semantics, minimum roles, hosted CI and production deployment remain separate acceptance work. The next feature milestone is source/capture-grounded callable SCRUB/RESILVER contract work. A production-representative soak can later refine budget confidence without treating sampling as a hard upper bound.

## Reproduction

From the repository root with an explicitly selected local Docker Desktop context:

~~~sh
validation_dir=$(mktemp -d /tmp/truenas-separated.XXXXXX)
docker --context desktop-linux build --platform linux/amd64 -f deployment/Dockerfile \
  -t local/truenas-rust-exporter:isolated-cgroups .
docker --context desktop-linux build --platform linux/amd64 -f deployment/Dockerfile \
  --target operational-test --output "type=local,dest=$validation_dir" .
python3 -B scripts/profile-isolated-cgroups.py --context desktop-linux \
  --image local/truenas-rust-exporter:isolated-cgroups --binaries "$validation_dir" \
  --output /tmp/truenas-isolated-cgroups.json
~~~

The raw controller output contains full numeric Docker memory-stat snapshots; the committed evidence keeps selected time-series counters and complete peak-sample memory stats. The controller stops/removes only its UUID-named containers and deletes its temporary synthetic configuration directory. Exported binaries/build cache and the explicitly requested output file remain available for review. No host trust, production infrastructure or global toolchain configuration is changed.
