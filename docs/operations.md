# Operations and deployment boundaries

Development uses synthetic fixtures and loopback servers only. MEMORY ALPHA (10.10.10.7) is strictly off-limits until an operator explicitly authorizes a specific interaction. No test is allowed to use production credentials. Automated tests remain offline. An explicit operator-authorized transport-only command is available; see memory-alpha-integration.md. Authenticated calls must wait for provisioned dedicated credentials.

Local: `docker compose -f deployment/compose.yaml config` then `docker compose -f deployment/compose.yaml up --build`. This explicit mock service exposes 9110 only inside its Docker network. Use docker compose exec with the built-in healthcheck for liveness; attach a local Prometheus service to that network to scrape truenas-exporter:9110. A developer may explicitly add a loopback host port mapping for local inspection. The mock stack is not a competing production stack.

Production examples are opt-in artifacts. Use a built, reviewed image and merge the example fragment into the existing observability project only after authorization. Paths resolve from that Compose project's directory, not from the Mac checkout. Readable secret/CA mounts must match UID/GID 10001; root:root 0600 files will not work. No Docker socket, privileged mode or TrueNAS installation. Certificate trust and credential setup must be performed by the operator through supported TrueNAS administration, after the role matrix is verified; do not send secrets through chat.

Run `docker compose config` on the eventual merged configuration before rollout. Verify its actual network name and service DNS. Append the sample scrape job to existing scrape_configs; never replace the existing file. No 9108/9109 port reuse, no Graphite removal or automatic dashboard import. Both jobs must remain during a separately authorized comparison.

Container is non-root, read-only, all capabilities dropped, no-new-privileges, with /healthz healthcheck. /readyz is not a restart trigger. Resource limits are illustrative until measured. TLS failures emit a safe category and trust/name guidance; keys, URLs, frames and remote errors are not logged. Repeated failures use bounded retries.

License: MIT. See the root LICENSE. Local linux/amd64 runtime checks pass through TN-15. RustSec and image scan results are historical until refreshed; medium/low findings remain tracked and captured appliance fixtures are still outstanding; see security-validation.md and validation.md.

The runtime has no shell or package manager. Use logs/metrics and `truenas-exporter healthcheck` for diagnosis; do not add tools to the deployed image. The runtime validation probe is exported and mounted only during local/CI tests. OpenSSL/Python are host-side test requirements, not runtime dependencies.

The integration fragment now consumes a read-only target JSON file plus distinct key and public Caddy CA mounts. Set TRUENAS_TARGETS_SOURCE, MEMORY_ALPHA_API_KEY_SOURCE and CADDY_CA_SOURCE on the Docker host; deployment/.env.example contains path examples only. Credentials are mounted at /run/secrets/memory-alpha-api-key and the public CA at /run/secrets/memory-alpha-caddy-root-ca.pem. Add each future server's entry and secret mounts explicitly. No host trust store or runtime image mutation is required.

The example fragment does not create a production network or account, and was not applied. Use paths appropriate to the eventual Docker host; the Mac's ~/.config path is not assumed to exist on the observability VM.


Container validation reports success after removing its temporary containers. A cleanup failure identifies affected containers and makes an otherwise successful run fail. If validation also fails, its original exception is retained and cleanup errors are written to stderr. Failed starts that leave no container are distinguished with a successful Docker list query. The script attempts cleanup of every name registered by its run.


## Integrated offline operational validation

The opt-in sustained system/realtime test and its container reproduction are documented in [integrated-operational-validation.md](integrated-operational-validation.md). It uses generated synthetic credentials and loopback TLS only, with two sockets per target, silence/outage/recovery and stalled-reader shutdown. Four sustained tests are intentionally ignored by ordinary cargo test and require explicit --ignored invocation. Child CPU/RSS and whole-cgroup resources have different scopes; neither a finite pass nor a capped run certifies long-term memory stability or production readiness.

The [phased memory profile](operational-memory-profile.md) adds integrated_memory_profile, including 30 seconds of warmed baseline, two minutes of scrape pressure, 45 seconds of cooldown and three recovery cycles under a 900-second deadline. It uses the same synthetic fixture; no appliance authorization or production key is needed.


[Separate-cgroup validation](isolated-cgroup-validation.md) uses scripts/profile-isolated-cgroups.py to run the fixture and exporter in separate containers/cgroups sharing only the network-disabled loopback namespace. It requires an explicit local Docker context and rejects real endpoints/credentials. Invoke this fourth sustained scenario through that controller; select exact test names for the three in-process scenarios instead of running every ignored operational test indiscriminately.
