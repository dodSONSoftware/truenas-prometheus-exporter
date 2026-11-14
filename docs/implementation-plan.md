# Implementation plan and acceptance checklist

## Current review status — November 13, 2026

Fourteen findings have implementation fixes. TN-02 awaits a captured, sanitized appliance response. At b2e9919, 45 Rust tests, six Python tests, formatting, strict Clippy and local linux/amd64 container checks passed. Earlier results below retain their original scope. Credential provisioning needs operator confirmation.

| Finding | Commit | Disposition |
| --- | --- | --- |
| TN-01 | 4be109e | Implemented and locally verified |
| TN-02 | pending | Waiting for appliance evidence |
| TN-03 | 0bfcab0 | Implemented and locally verified |
| TN-04 | 82588c3 | Implemented; finite full-process operational validation passed |
| TN-05 | 4750a91 | Implemented and locally verified |
| TN-06 | be2b94f | Implemented and locally verified |
| TN-07 | 8009702 | Implemented; finite full-process operational validation passed |
| TN-08 | 9a0b534 | Implemented and locally verified |
| TN-09 | b0ec1fe | Implemented and locally verified |
| TN-10 | b8ae644 | Implemented and locally verified |
| TN-11 | a036f4d | Implemented and locally verified |
| TN-12 | 9073311 | Implemented and locally verified |
| TN-13 | 9b22c73 | Implemented and locally verified |
| TN-14 | a782c14 | Implemented and locally verified |
| TN-15 | b2e9919 | Implemented and locally verified |

The operational continuation verifies sustained independent TLS connections, outage recovery, full-process shutdown under backpressure, Rust 1.88 and refreshed security gates. Hosted CI requires an authorized GitHub run. Real-appliance and eventual deployment acceptance remain open. M3 remains gated on authenticated compatibility under the existing plan.

Accepted findings: missing captured fixtures; pin API evidence; enumerate protocol allowlist; resolve scan names; deterministic readiness/partial failure; complete source mappings; use one root repository. Their contracts are in compatibility.md, api-contract.md, architecture.md and metric-catalog.md.

## M1
- [x] Single root repository and one Rust package, fail-fast typed config.
- [x] HTTP operational metrics, liveness/readiness, explicit offline mock mode.
- [x] Deterministic pool snapshot failure/expiry state and tests.
- [x] Docker/Compose examples, root docs, configuration inventory.
- [x] Local fmt, strict Clippy, 45 Rust tests, six Python tests and process checks through TN-15.
- [x] linux/amd64 Docker build, mock Compose configuration and hardened local runtime checks. CI workflow supplied but not executed.

## M2 (only after API contract documentation)
- [x] Document API wire allowlist, auth, limits and compatibility assumptions.
- [x] Verified TLS/private CA and typed auth/pool query.
- [x] Bounded request correlation, deadlines, reconnect and cancellation.
- [x] Synthetic pool fixture and local WebSocket failure tests.
- [ ] Captured sanitized 25.04.2.6 fixtures: NOT SUPPLIED; separate acceptance gate.

## M3 and later
- [ ] Exact system/version/uptime/memory sources and runtime compatibility gate.
- [ ] Realtime wire fixtures, subscription recovery, per-group validation and silence expiry.
- [ ] General scan metrics (SCRUB and RESILVER), verified callable schema.
- [ ] Container outage/recovery, least privilege and runtime resource validation.
- [ ] P1 collectors only after P0 contracts are established.
- [ ] Separate operator-authorized appliance validation and production integration.

No implementation milestone is fully validated until its container checks pass. Synthetic tests are not appliance compatibility evidence. Milestone commits are authorized by the current user instruction and follow git-commit-message-instructions.md. Pushing and production rollout require separate authorization.

## Item 1: validation before M3

- [x] MIT license and requested copyright in LICENSE, package metadata and runtime image.
- [x] Rust 1.88 formatting, strict Clippy and all 16 tests.
- [x] linux/amd64 release image build, mock Compose validation and hardened runtime checks on local Docker Desktop.
- [x] RustSec audit with zero vulnerabilities/warnings after removing an unmaintained wrapper; CI gate added.
- [x] Image vulnerability scan completed and findings recorded in security-validation.md.
- [x] Replace Debian slim with signed, digest-pinned distroless and repeat checks: no high/critical findings, no exclusions. Track remaining medium/low findings.
- [x] Create fixtures/truenas-25.04.2.6/ and document captured-fixture intake.
- [ ] Supply genuine sanitized 25.04.2.6 payloads with provenance, then validate offline.

## Delivery status

M1/M2 functional work and fourteen review implementations pass recorded local checks. The preceding release is 0.0.13 with fourteen local commits ahead of origin/main. TN-02 remains open and M3 has not started. Security scan results are historical until refreshed. See validation.md and security-validation.md.

## MEMORY ALPHA proxy integration and multiple targets

- [x] Update endpoint examples to wss://memory-alpha.home.lab/api/current.
- [x] Export the verified public Caddy root outside Git; native and container WSS upgrade pass with verification enabled. Container negative trust test rejects the untrusted CA.
- [x] Add a transport-only diagnostic that never loads/sends API credentials.
- [x] Add bounded multi-target configuration, independent workers/caches, stable target labels, combined metadata and partial-outage readiness semantics.
- [x] Validate multi-target credentials/CA isolation, identical pool names, independent expiration and log redaction against offline TLS mock servers.
- [ ] Dedicated prometheus_exporter account and key provisioning by the operator.
- [ ] Authenticated login, permission verification and live pool.query capture on 25.04.2.6.
- [ ] Repeat authenticated API collection inside Docker; validate real secret-file ownership.

M3 backend/system/realtime implementation is held until authenticated compatibility is established. No credentials were substituted, no API-key file was created, and no infrastructure was modified. See memory-alpha-integration.md.

Offline operational continuation passed on the Mac and its local linux/amd64 Docker runtime. See the final measurements in validation.md and the refreshed signature/security evidence in security-validation.md. TN-02 is the prerequisite still awaiting the operator-guided account and capture workflow.
