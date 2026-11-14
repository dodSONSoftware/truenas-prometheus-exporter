# Integrated system/realtime operational baseline

November 14, 2026. This is finite, synthetic, full-process acceptance on native macOS and local Docker Desktop. It is not appliance acceptance, a long-duration soak, a universal resource-budget guarantee or a production deployment. Production code, dependencies, defaults and base-image pins did not change.

## Reproducible workload

application/tests/operational.rs now shares one bounded loopback TLS fixture between the original pool-only scenario and a new explicitly ignored integrated scenario:

- 16 configured targets; two independently authenticated/version-checked WSS sockets each, with exactly 32 active sessions after readiness and after full recovery.
- Per target: 256 pools with 128-character names, 257 CPU entries (aggregate plus 256 cores), and 128 up interfaces with 64-character names. Cardinality/pool/interface-name bounds are saturated; target names and system identities are shorter than their limits. Events are entirely synthetic, not appliance captures.
- Pool/system polling every second, API timeout one second, cache TTL three seconds; exact realtime subscription interval two seconds and realtime TTL three seconds.
- Four concurrent metrics clients with 50 ms delay after each completed request; 30 one-second health observations alongside actual polling/event delivery. Successful scrapes require a complete EOF; admission-pressure 503 responses must report encoder busy.
- Eight seconds of realtime silence, while polling and ping/pong continue. Realtime CPU/memory/network/ARC-size measurements must expire and readiness must fail while all 16 system totals and all 4096 pool sizes remain present. The fixture then restores events and requires readiness recovery within 70 seconds. Scrape load is drained before detailed metric inspection.
- Fifteen seconds of full transport outage: sessions close and reconnect attempts fail. Pool/system/realtime measurement families checked by the test must disappear after expiry; last-success metadata remains. All targets must recover within 70 seconds, with exactly 32 active sessions again.
- SIGTERM while a metrics reader with a 1 KiB receive window remains stalled. All 16 final owned subscription IDs must be unsubscribed; the exporter must exit successfully within five seconds and server sessions must drain. The global integrated-test deadline is 240 seconds.

The fixture allowlist consists only of auth.login_ex, system.info, pool.query, exact core.subscribe and owned core.unsubscribe. All certificates and keys are generated synthetic test material in a temporary directory. No production credentials or appliance endpoint are read. The original pool-only test remains separately runnable; adding this scenario does not silently make ordinary cargo test run a sustained benchmark.

## Native run

Native arm64 macOS 27.2.1, Rust 1.88 release build:

~~~sh
cargo +1.88.0 test --locked --release --manifest-path application/Cargo.toml \
  --test operational integrated_realtime_outage_recovery_and_stalled_sigterm \
  -- --ignored --nocapture
~~~

A temporary external monitor sampled only the printed exporter child PID using /bin/ps -o rss=,time= every 500 ms. RSS KiB were multiplied by 1024; cumulative CPU time differences were divided by monotonic observation time. It observed 147 samples over 74.097 seconds, 30.52 CPU seconds, about 41.19% of one core on average, and a sampled RSS peak of 489,865,216 bytes (467.17 MiB). These are sampled process values, not an exact kernel high-water mark, a steady-state plateau, or whole-harness resources; startup/exit gaps may be missed. No native memory cap was applied. Some validation runs overlapped with other local work on the same development machine; these are not controlled cross-platform performance comparisons.

The native integrated scenario passed in 74.44 seconds: 883 successful scrapes, 1459 expected encoder-busy responses, 760 pool requests, maximum sampled health latency 1 ms and SIGTERM-to-exit 2003 ms. There were 64 cumulative subscriptions across initial, silence-retry and outage-recovery connections, and 16 final owned unsubscribe requests verified by the fixture. Closed transports reclaim their subscriptions; they are not counted as explicit unsubscribe acknowledgements.

The original pool-only regression also passed on native Rust 1.88 release: 63.41 seconds, 1419 successful scrapes, 1523 expected busy responses, 582 pool requests and 2006 ms shutdown. The fixture now also tracks active-session cleanup; these observations do not replace the historical workload records.

## Limited Docker run

The operational-test Docker stage exports the actual release exporter plus its process-test binary; these are mounted into the pinned distroless runtime. Runtime UID/GID is 10001:10001, read-only root, no capabilities, no-new-privileges, 128 MiB memory with swap disabled, half a CPU, and a 16 MiB /tmp tmpfs. Network mode none permits only internal loopback. The final runtime image is local/truenas-rust-exporter:integrated-operational; image ID and final observed results are recorded below.

~~~sh
validation_dir=$(mktemp -d /tmp/truenas-integrated.XXXXXX)
docker --context desktop-linux build --platform linux/amd64 -f deployment/Dockerfile \
  -t local/truenas-rust-exporter:integrated-operational .
docker --context desktop-linux build --platform linux/amd64 -f deployment/Dockerfile \
  --target operational-test --output "type=local,dest=$validation_dir" .
docker --context desktop-linux run --rm --platform linux/amd64 \
  --network none --memory 128m --memory-swap 128m --cpus 0.5 \
  --read-only --tmpfs /tmp:rw,size=16m --cap-drop ALL \
  --security-opt no-new-privileges:true \
  --mount "type=bind,src=$validation_dir,dst=/validation,readonly" \
  --mount "type=bind,src=$validation_dir/truenas-exporter,dst=/build/application/target/release/truenas-exporter,readonly" \
  --entrypoint /validation/operational local/truenas-rust-exporter:integrated-operational \
  integrated_realtime_outage_recovery_and_stalled_sigterm --ignored --nocapture
~~~

Linux-only instrumentation samples the child /proc/PID/status VmRSS and /proc/PID/stat user+system CPU ticks every 500 ms until just before SIGTERM. The test prints sample count, sampled RSS peak, CPU delta, clock ticks/second and elapsed observation time. This is separate from memory.peak and cpu.stat, which account for the entire fresh cgroup including the mock WSS server, TLS work, scrape clients, test harness and file/cache/tmpfs charges. Cgroup memory peak is not child RSS; the two quantities must not be subtracted to infer exact fixture memory. The CPU counter is not exporter-only CPU.

Final instrumented run (Rust 1.88 release, linux/amd64 under local Docker Desktop):

| Observation | Result |
| --- | ---: |
| Scenario duration | 74.91 seconds |
| Successful /metrics responses | 253 |
| Expected encoder-busy responses | 1875 |
| Pool requests | 778 |
| Maximum sampled /healthz latency | 40 ms |
| SIGTERM-to-exit | 2022 ms |
| Cumulative subscriptions / final owned unsubscribe requests | 64 / 16 |
| Exporter child samples / observation interval | 145 / 72.455 seconds |
| Exporter sampled RSS peak | 62,930,944 bytes (60.02 MiB) |
| Exporter CPU delta | 1903 ticks at 100 ticks/second = 19.03 CPU seconds |
| Exporter average CPU over observation | 26.26% of one core |
| Whole-cgroup memory peak | 126,873,600 bytes (121.00 MiB) |
| Whole-cgroup CPU counter | 21.498589 CPU seconds, approximately 28.70% of one core over scenario duration |
| Cgroup throttled time | 19.641827 seconds across 394 throttled periods |

Image ID: sha256:5baf94cc3fbd015f518b2062636153069b939a0b9a610ad577f0b5530626a90d. Exported exporter binary SHA-256: 6ac98f2d93c54600576b0b90a8843925a4fc27da0cda9d5f8281f0d0c220eb55. Final instrumented test binary SHA-256: 0f3036c41cc5a31693e7dd18bfb187ab13fad9f6e480dc6d7e4d818d75c9eade. The test invokes the mounted release exporter, not a new production capture/debug command.

An initial container pass before adding child-only sampling took 75.80 seconds with 130,547,712 bytes (124.5 MiB) cgroup peak. The lower final peak is run variation, not a production-code optimization. The final cap margin is about 7.0 MiB; the earlier margin was 3.5 MiB. Both are whole-workload observations and do not prove production headroom. The named test container was automatically removed after exit.

The fixture checks every owned unsubscribe request against that connection's ID and responds with a null result. Its cleanup counter counts received/validated requests, not an independent observation that the client consumed each acknowledgement; response correlation/cleanup timeout behavior has separate protocol coverage.

## Acceptance and remaining work

The finite functional gate passes only when readiness/expiry/isolation/recovery/owned-cleanup assertions succeed. Resource observations retain their platform/workload scope. A capped run finishing without OOM does not establish comfortable headroom or long-term stability. In particular, the high native sampled RSS requires profiling before asserting a general low-memory claim; the earlier container run also approached the 128 MiB cgroup cap.

Next resource work is a longer steady-state and repeated recovery profile, distinguishing allocator retention from growth and attributing response/snapshot buffers with controlled scrape pressure. Use child measurements and cgroup measurements separately. Do not change production resource limits or collector defaults based solely on this single finite workload. Independent appliance numerical/chart semantics, least privilege, hosted CI and eventual deployment remain separate gates; see realtime-numerical-review.md.
