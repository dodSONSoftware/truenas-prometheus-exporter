# Implementation plan and acceptance checklist

Accepted findings: missing captured fixtures; pin API evidence; enumerate protocol allowlist; resolve scan names; deterministic readiness/partial failure; complete source mappings; use one root repository. Their contracts are in compatibility.md, api-contract.md, architecture.md and metric-catalog.md.

## M1
- [x] Single root repository and one Rust package, fail-fast typed config.
- [x] HTTP operational metrics, liveness/readiness, explicit offline mock mode.
- [x] Deterministic pool snapshot failure/expiry state and tests.
- [x] Docker/Compose examples, root docs, configuration inventory.
- [x] Local fmt, clippy, 16 tests, release build and offline process smoke checks.
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

No implementation milestone is fully validated until its container checks pass. Synthetic tests are not appliance compatibility evidence. No production rollout, commit or push is authorized by this plan.

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

M1/M2 functional work passes local Rust 1.88 and linux/amd64 container checks. The runtime high/critical security gate passes. Item 1 remains open for captured appliance evidence. M3 is not started. No commits, pushes or production deployments were made. Subsequent explicitly authorized transport-only endpoint tests are recorded below. See validation.md and security-validation.md.

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
