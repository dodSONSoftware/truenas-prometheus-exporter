# Phased operational memory profile

November 14, 2026. This extends the finite integrated baseline with a longer synthetic steady-state/scrape/cooldown comparison and three outage/recovery cycles. Production code, dependencies, defaults and resource configuration are unchanged. No appliance call or production credential is used. Findings apply to this finite workload and do not certify an indefinite leak-free process.

## Workload and measurement

The explicitly ignored integrated_memory_profile scenario in application/tests/operational.rs reuses the same generated loopback WSS fixture: 16 targets, 32 active authenticated/version-gated connections, 256 pools, 257 CPU entries and 128 interfaces per target, with pool/interface labels at their bounds. Polling is every second; realtime interval is two seconds; API timeout one second and TTLs three seconds. Real default configurations remain unchanged.

The test has a 900-second outer deadline and runs these phases:

1. Initialization and selected metric cardinality checks, then 30 seconds without continuous scrape clients. Initial validation scrapes mean this is a warmed baseline, not a pristine never-scraped process.
2. Approximately 120 seconds of four clients, each waiting 50 ms after a completed metrics request. Complete 200 responses and expected encoder-busy 503s are counted separately. Health checks continue each second and polling must progress.
3. Stop and join all continuous clients, then 45 seconds of cooldown with normal collection still running.
4. Three cycles of eight seconds of full transport outage, expiry/readiness assertions, restored service with readiness recovery within 70 seconds, exactly 32 active sessions again, then 20 quiet seconds. Outage-labelled measurement phases include the reconnection wait until all targets become ready; recovery-labelled phases begin after readiness is restored. Occasional assertion scrapes remain; these are not literally zero-HTTP phases.
5. Final metric checks, an open stalled metrics reader and successful bounded SIGTERM, including 16 connection-owned unsubscribe requests verified by the fixture and session drainage. The final recovery measurement can include the final assertion scrape/stalled-response setup immediately before the sampler stops; its last/peak values are not a perfectly isolated quiet-only window.

Test-only sampling runs every 500 ms. On macOS it invokes /bin/ps -o rss=,time= for the exporter child; on Linux it reads only the child's /proc status/stat and converts CPU ticks using sysconf. Each contiguous phase prints JSON containing sample count, observation seconds, first/last/minimum/peak RSS and CPU delta. Values near phase transitions are approximate to the sample cadence and command latency. Samples are bounded by the scenario deadline. RSS is resident memory, not an accounting of live Rust objects or a precise allocation high-water mark.

Linux also prints whole-cgroup memory.peak/cpu.stat. Those include the test fixture, TLS server, clients, page-cache/tmpfs charges and exporter, so they cannot be substituted for child RSS or subtracted from it to infer exact fixture allocation. Native and Docker performance is not a controlled cross-platform comparison; no allocator or ownership diagnosis follows solely from different RSS values.

## Reproduction

~~~sh
cargo +1.88.0 test --locked --release --manifest-path application/Cargo.toml \
  --test operational integrated_memory_profile -- --ignored --nocapture
~~~

For the local container, follow the operational-test binary export and capped runtime command in [integrated-operational-validation.md](integrated-operational-validation.md), substituting integrated_memory_profile as the exact test filter. The actual profile used UID/GID 10001:10001, network none, read-only root, dropped capabilities, no-new-privileges, 16 MiB /tmp tmpfs, 128 MiB memory with swap disabled, half a CPU and linux/amd64 under Docker Desktop. Synthetic certificates/keys are generated and removed by the fixture. The native run used arm64 macOS and Rust 1.88 release.

## Observed results

Both profiles passed all functional assertions. Native duration was 328.72 seconds; container duration was 330.50 seconds. The fixture verified 32 active sessions after each recovery, 16 final owned unsubscribe requests and complete session drainage. This counter validates requests/ownership; it does not independently observe client consumption of each null acknowledgement.

RSS below is **minimum / last / peak**, in MiB, for each phase. The raw numerical evidence, sample counts, phase durations and CPU deltas are in [the evidence record](evidence/operational-memory-profile-2026-11-14.json).

| Phase | Native exporter RSS MiB (min / last / peak) | Container exporter RSS MiB (min / last / peak) |
| --- | ---: | ---: |
| baseline_no_scrapers | 39.53 / 39.53 / 45.45 | 25.19 / 26.07 / 40.49 |
| scrape_load | 49.55 / 374.83 / 471.39 | 51.98 / 60.55 / 60.55 |
| cooldown_no_scrapers | 53.38 / 53.41 / 413.89 | 47.82 / 47.82 / 60.55 |
| recovery_1 | 53.61 / 53.62 / 53.62 | 41.75 / 41.75 / 41.75 |
| recovery_2 | 53.64 / 53.64 / 53.64 | 41.76 / 41.77 / 41.77 |
| recovery_3 | 53.67 / 53.67 / 53.67 | 41.77 / 54.52 / 54.52 |

Native RSS peaked at 471.39 MiB during scrape load, then ended cooldown at 53.41 MiB. Quiet recovery phase minima were 53.61, 53.64 and 53.67 MiB. There was no large accumulating per-cycle resident footprint over these three cycles. The post-cooldown footprint remained about 13.9 MiB above the warmed baseline end, so this is not a claim of zero retention or proof of allocation ownership. The high scrape-time resident peak remains relevant for native maximum-cardinality use.

Container child RSS peaked at 60.55 MiB during scrape load. It ended cooldown at 47.82 MiB, and quiet recovery minima were about 41.75, 41.76 and 41.77 MiB. The third phase ends at 54.52 MiB and includes final assertion/stalled-reader metric setup; this cannot be treated as a clean 20-second quiet end value. Coarse phase summaries do not identify a specific retained buffer or allocator. No new allocator, shrink policy or cache rewrite is justified by this evidence alone.

During sampled scrape-load intervals, native child CPU was 93.82 seconds over 119.84 seconds (about 78.3% of one core), versus container child 56.14 seconds over 119.86 seconds (about 46.8% of one core under the half-core cap). The requests completed at different rates; this is not a platform efficiency comparison.

| Whole-scenario observation | Native | Container |
| --- | ---: | ---: |
| Successful scrapes | 2717 | 806 |
| Expected encoder-busy responses | 4662 | 5975 |
| Pool requests | 4488 | 4466 |
| Maximum sampled health latency | 9 ms | 37 ms |
| SIGTERM-to-exit | 2005 ms | 2016 ms |
| Cumulative subscriptions / final owned requests | 64 / 16 | 64 / 16 |

The whole container cgroup reached exactly 134,217,728 bytes, its **128 MiB cap**, while the processes completed successfully. This includes fixture/client/cache charges. No cgroup memory.events evidence was collected, so peak alone does not characterize reclaim/pressure/OOM-event history. It does not establish production headroom or show that the exporter alone requires 128 MiB. Whole-cgroup CPU usage was 73.000067 seconds and reported throttled time 56.137137 seconds. Independent exporter/fixture cgroup attribution is the next resource step before claiming a production memory budget.

No production correction was made: the profile did not reproduce a functional failure or a large resident increment per reconnect. It showed a substantial scrape-associated native RSS peak and a capped combined workload with unproven headroom. The isolated runtime uses the same production binary as the previous milestone; source/binary identities and actual runtime image are recorded in the evidence JSON.


## Interpretation and remaining acceptance

Interpret the warmed baseline, load, cooldown and restored phases separately. A load-associated increase that remains during cooldown is consistent with retention but does not identify its cause. Flat/reduced RSS during a few recovery cycles is evidence against per-cycle resident growth in this run, not proof that every object is freed or that longer runs cannot grow. First/last/peak phase summaries are coarse observations, not continuous allocation traces or a statistically established steady-state plateau.

This profile can identify where a follow-up heap/allocator experiment is warranted. Do not add a new allocator, adjust production limits, reduce supported cardinalities or change response semantics without a demonstrated cause and focused verification. Existing independent live numerical/chart, least-privilege, hosted CI and deployment gates remain separate. The next feature milestone is callable SCRUB/RESILVER contract review; any resource issue demonstrated here must retain an explicit disposition alongside that work.
