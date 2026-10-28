# Changelog

## Unreleased - 0.1.0 development

- Adopt the MIT license, Copyright (c) 2026 dodson Software ( dodson labs ).
- Validate the Rust 1.88 MSRV and remove the unmaintained rustls-pemfile wrapper in favor of rustls-pki-types PEM parsing.
- Add a strict RustSec audit CI gate and reserve fixtures/truenas-25.04.2.6/ for captured sanitized evidence.

- Initial M1/M2 implementation: offline mock mode, exporter endpoints, bounded pool snapshots, verified TLS JSON-RPC client and local test fixtures.
- Document API assumptions, freshness semantics, deployment boundaries and deferred realtime/system collectors.
- Pass local Rust 1.88, linux/amd64 image build and hardened container checks. Captured appliance fixtures remain outstanding; runtime high/critical findings are resolved by the distroless change.
- Digest-pin both container bases and include the MIT license in the runtime image.

- Replace Debian slim with signature-verified, digest-pinned distroless cc-debian13 while retaining UID/GID 10001 and the executable healthcheck.
- Replace shell-based container checks with a test-only mounted probe; cover synthetic secret/CA mounts and network-isolated outage readiness.
- Reduce detected OS packages from 91 to 14 and high/critical scan findings to zero. Retain 23 medium/8 low findings in the security report and add a pinned Trivy CI gate.

- Add explicit verified WSS transport diagnostics without credential loading or API calls; validate the MEMORY ALPHA Caddy endpoint from host and container.
- Support multiple independently configured TrueNAS targets with file-only credentials, per-target CA trust, isolated collection/freshness and stable target labels.
- Add offline multi-target process tests and read-only target/CA/credential mount examples. Authenticated live validation remains blocked on operator provisioning.
