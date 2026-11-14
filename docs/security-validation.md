# Item 1 security validation

**Scope notice (November 14, 2026):** Security results below belong to their recorded image/database/date. Current runtime base is pinned in deployment/Dockerfile; later tooling/docs milestones have not changed dependencies/base pins or performed new scans. Captured pool/system/realtime evidence now exists; minimum-role, final-candidate CI/security and deployment acceptance remain open. See [project-status.md](project-status.md).

## Latest recorded refresh — November 13, 2026

The runtime digest is now sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2. Cosign verified the exact digest with issuer https://accounts.google.com and identity keyless@distroless.iam.gserviceaccount.com, including signature claims and transparency evidence.

A refreshed Trivy 0.76.1 database (updated 2026-11-14 01:05:40 UTC, November 13 locally) reported two HIGH zlib1g package findings against the former runtime digest. The findings were CVE-2026-76844 and CVE-2026-85091, both with a fixed Debian package version identified by the scanner. The replacement runtime removes both HIGH findings with the same database. The final image is sha256:ea5518f6a824f6333e7ed585f20a9b7bc7270e299d4bc422b9a5e5117540c1dc, tagged local/truenas-rust-exporter:operational.

The explicit HIGH/CRITICAL gate passed on the final image without exclusions. Current package findings are 0 CRITICAL, 0 HIGH, 39 MEDIUM, 12 LOW and 2 UNKNOWN across 14 packages. Lower-severity findings remain visible. Full reports were saved locally to /private/tmp/truenas-current-scan.json and /private/tmp/truenas-final-scan.json. Scan findings describe this image/database combination and do not establish exploitability of each package in the exporter.

cargo-audit 0.22.2 passed with --deny warnings against 173 locked dependencies using advisory database commit 4ef2df8e806a65e2f1ecc14f56f51249062b23d6. No dependency version changes or advisory exclusions were introduced.

## Historical October 27 evidence

Date: 2026-10-27 (America/Chicago). Initial security checks used local mocks. Subsequent explicitly authorized transport-only Caddy endpoint tests are recorded in memory-alpha-integration.md; no authenticated NAS API call or production deployment was performed.

## Historical October result (base superseded by the later recorded refresh)

The runtime now uses `gcr.io/distroless/cc-debian13:nonroot`, pinned to `sha256:3ee00291cf386ec52a552e6a858d555debd74776eb4bfa53fa3df2cd6677515e`. The linux/amd64 image passes the explicit **HIGH/CRITICAL vulnerability gate with no exclusions**. Medium/low findings remain visible below. This is not a guarantee of vulnerability absence or permission to deploy.

The prior Debian slim runtime is superseded. Its high/critical gate failure was resolved by removing unneeded OS components through a maintained minimal runtime, rather than suppressing CVEs.

## Runtime selection and provenance

- Shared-library inspection of the exporter found `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, and `/lib64/ld-linux-x86-64.so.2`. The distroless cc image provides the glibc/libgcc runtime used for Rust binaries; no musl migration or application-code change was needed.
- Cosign 3.1.4 verified the pinned upstream image's claims, certificate chain, and transparency-log inclusion. Verified issuer: `https://accounts.google.com`; subject: `keyless@distroless.iam.gserviceaccount.com`. One signature matched the exact pinned digest. No insecure signature bypass was used.
- The publisher's documented Rust example uses cc-debian13; its support policy tracks Debian's standard support timeline.
- UID/GID remains `10001:10001`. The exporter healthcheck and entrypoint remain direct executable invocations. There is no runtime APT installation, shell, package manager or extra probe executable.

Sources:
- https://github.com/GoogleContainerTools/distroless/blob/main/cc/README.md
- https://github.com/GoogleContainerTools/distroless/blob/main/examples/rust/Dockerfile
- https://github.com/GoogleContainerTools/distroless/blob/main/SUPPORT_POLICY.md
- https://github.com/GoogleContainerTools/distroless#how-do-i-verify-distroless-images

## Rust dependency audit

RustSec audit using cargo-audit 0.22.2 and `--deny warnings`: **0 vulnerabilities and 0 warnings**, 173 dependencies. Database commit: `99670cfc0bc574433034b75876a4b791ccaa2efb` (updated `2026-10-27T11:02:42+01:00`).

The earlier unmaintained rustls-pemfile warning was resolved by using rustls-pki-types PEM parsing. No dependency advisory ignore list was added. Source: https://rustsec.org/advisories/RUSTSEC-2025-0134.html

## Image comparison

Trivy 0.76.1 scanned local image archives with `--scanners vuln`. The comparison uses the same downloaded vulnerability database; the distroless scan used `--skip-db-update`. Image/source contents were not uploaded. Current scan time: `2026-10-27T22:51:29.847164-05:00`.

| Measure | Previous Debian slim | Current distroless |
| --- | ---: | ---: |
| OS packages detected | 91 | 14 |
| Docker-reported image size (MiB) | 129.3 | 49.6 |
| CRITICAL package findings | 1 | 0 |
| HIGH package findings | 56 | 0 |
| MEDIUM package findings | 87 | 23 |
| LOW package findings | 85 | 8 |
| UNKNOWN package findings | 1 | 0 |
| Unique CVEs | 100 | 25 |

Sizes are Docker-reported local image sizes, not download sizes. A source-package CVE can be reported against multiple binary packages. Trivy found no Rust package metadata in the stripped runtime; Rust dependencies are covered by the lockfile audit above.

Initial distroless image ID: `sha256:0e301e64d71c1c70582de1aeac8e0a97fc17240acfeb5079b88a3b6e642710e6`, retained locally as `local/truenas-rust-exporter:distroless-validation`. The validation tag now refers to the newer multi-target build recorded below. Prior image ID: `sha256:17068a069a04be131e78875bc2dcd029cf00bc25de8b7171f307fa37c945866f`, retained as `:debian-validation` for comparison.

## Remaining medium/low findings

No fixed version is listed for these remaining findings in the scanned database. They have not been suppressed. Track relevant upstream fixes and re-scan on base updates; package detection alone does not establish runtime exploitability.

| CVE | Severity | Reported binary packages |
| --- | --- | --- |
| CVE-2010-4756 | LOW | libc6 |
| CVE-2018-20796 | LOW | libc6 |
| CVE-2019-1010022 | LOW | libc6 |
| CVE-2019-1010023 | LOW | libc6 |
| CVE-2019-1010024 | LOW | libc6 |
| CVE-2019-1010025 | LOW | libc6 |
| CVE-2019-9192 | LOW | libc6 |
| CVE-2026-102010 | MEDIUM | gcc-14-base, libgcc-s1, libgomp1, libstdc++6 |
| CVE-2026-18374 | MEDIUM | libc6 |
| CVE-2026-19499 | MEDIUM | libc6 |
| CVE-2026-19542 | MEDIUM | libc6 |
| CVE-2026-27171 | MEDIUM | zlib1g |
| CVE-2026-5435 | MEDIUM | libc6 |
| CVE-2026-6238 | MEDIUM | libc6 |
| CVE-2026-6368 | MEDIUM | libc6 |
| CVE-2026-6791 | MEDIUM | libc6 |
| CVE-2026-77117 | MEDIUM | libc6 |
| CVE-2026-80489 | MEDIUM | libc6 |
| CVE-2026-85091 | MEDIUM | zlib1g |
| CVE-2026-8674 | MEDIUM | libc6 |
| CVE-2026-86805 | MEDIUM | libc6 |
| CVE-2026-89092 | MEDIUM | libc6 |
| CVE-2026-95619 | MEDIUM | gcc-14-base, libgcc-s1, libgomp1, libstdc++6 |
| CVE-2026-95818 | MEDIUM | libc6 |
| CVE-2026-97399 | LOW | libc6 |

## Runtime validation

The same linux/amd64 release binary runs under the Mac's local Docker Desktop Linux VM/emulation. Both mock mode and a network-disabled normal-mode outage case pass:

- HTTP liveness/readiness/metrics and Docker's actual healthcheck scheduling.
- UID/GID 10001, all capability sets zero, no-new-privileges, and an actual EROFS write rejection.
- Absence of shell/package-manager utilities, readable MIT license and CA bundle.
- Synthetic key/private-CA mounts readable by the runtime UID; normal-mode startup parses the CA and remains live while its loopback-only endpoint is unavailable. Readiness correctly returns 503.
- Clean SIGTERM exit and removal of ephemeral test containers.

A standalone Rust probe replaces shell-based assertions. `validation-probe` is a separate Docker build target exported to a temporary host directory and bind-mounted read-only into test containers. It is never copied into the shipped runtime. Synthetic test material is publicly readable by design; real credential ownership remains an operator validation task.

All 16 Rust 1.88 tests pass, including local TLS trust/name rejection and authentication tests. Those TLS handshake tests run natively; the container-specific outage check validates CA/credential mounting and configuration, not an end-to-end successful WSS session. No actual appliance compatibility is claimed.

## Reproduce and maintain

From the repository root, on an authorized local daemon:

1. Build: `docker --context desktop-linux build --platform linux/amd64 -f deployment/Dockerfile -t local/truenas-rust-exporter:validation .`
2. Runtime checks: `python3 scripts/check-container.py --context desktop-linux` (requires Python 3 and OpenSSL on the host; automatically exports the probe).
3. Rust audit: `cargo audit --file application/Cargo.lock --deny warnings`.
4. Export: `docker --context desktop-linux image save -o /tmp/truenas-distroless-image.tar local/truenas-rust-exporter:validation`.
5. Full report: `trivy image --input /tmp/truenas-distroless-image.tar --scanners vuln --skip-version-check`.
6. Gate: add `--severity HIGH,CRITICAL --exit-code 1`. Do not add ignore-unfixed or broad CVE exceptions.

CI runs the shell-free runtime script and a Trivy 0.76.1 HIGH/CRITICAL gate through the official action pinned to an immutable commit. The local equivalent gate passed; GitHub CI itself has not been executed.

Digest pinning fixes the runtime filesystem, including certificates and packaged libraries. It also means security updates require an intentional digest refresh, signature verification, rebuild, regression checks and re-scan. The Rust builder remains separately pinned. No mutable APT install remains in the runtime stage.

The container high/critical gate is closed for this image/database snapshot. Captured fixtures were outstanding at this historical checkpoint and subsequently completed for pool/system/realtime; minimum appliance roles and deployment acceptance remain open. Production rollout still requires explicit authorization.

## Multi-target rebuild

After the transport diagnostic and multi-target implementation, the new image ID is `sha256:2dc783768b04a75364f5efc84070b74d516789c30daab01584591cae1c293ec6` (local/truenas-rust-exporter:multi-target-validation and :validation). Docker-reported size is 52,580,341 bytes, about 50.1 MiB. The base digest and dependencies are unchanged.

A full fresh scan invocation of Trivy 0.76.1 again detects 14 OS packages and 0 critical/0 high/23 medium/8 low package findings. Strict cargo-audit again returns 0 vulnerabilities and 0 warnings. No scan exclusions or inline secrets were added. The earlier distroless comparison remains historical evidence for the initial runtime replacement.

Transport-only live tests mount the verified public Caddy CA read-only and never provide API credentials. Positive host/container WSS upgrades and the container's negative trust test establish client TLS behavior independently of macOS native trust. They do not establish authentication, account permissions, backend version or pool schema compatibility.

## November 14 — local 0.4.2 candidate

Local image identity sha256:a658f618c61293f675c4f20847a7cc5e545aff506b40bbd0a087bfcb2fb796cc passed the existing synthetic hardening controller on desktop-linux. cargo-audit loaded 1509 advisories and scanned 173 locked dependencies with --deny warnings successfully. Trivy 0.76.1 HIGH,CRITICAL image gate found zero selected-severity findings across 14 Debian 13.7 packages. This filtered run makes no updated medium/low count claim. It detected no language-specific files. These results apply to this local image/database snapshot.
