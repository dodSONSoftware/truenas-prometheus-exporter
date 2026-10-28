# Project instructions

- Follow TrueNAS_Codex_Handoff.md and the clarified contracts in docs/.
- One repository at this directory; one Cargo package in application/. No nested repositories.
- MEMORY ALPHA (10.10.10.7) is off-limits: no network access, API calls, probes, or live tests without explicit operator authorization. Automated tests use loopback servers and synthetic fixtures. Manual live transport tests require explicit operator authorization; authenticated calls additionally require operator-provisioned dedicated read-only credentials.
- No production deployment, infrastructure edits, credentials in Git, or live dashboard changes.
- Keep API methods explicitly allowlisted, transport verified, memory bounded, logs redacted, and metrics cached with finite monotonic TTLs.
- Run cargo fmt --check, cargo clippy --all-targets -- -D warnings, cargo test --locked, and a container build when available. Report unrun validation.
- Do not commit, push, or publish without explicit authorization.
