# Build from the repository root: docker build -f deployment/Dockerfile .
FROM rust:1.88.0-bookworm@sha256:af306cfa71d987911a781c37b59d7d67d934f49684058f96cf72079c3626bfe0 AS build
WORKDIR /build/application
COPY application/Cargo.toml application/Cargo.lock ./
COPY application/src ./src
RUN cargo build --locked --release

# Explicitly exported full-process test binaries; excluded from the runtime.
FROM build AS operational-build
COPY application/tests ./tests
RUN cargo test --locked --release --no-run --test operational
RUN for test in target/release/deps/operational-*; do if [ -x "$test" ]; then cp "$test" /operational; fi; done

FROM scratch AS operational-test
COPY --from=operational-build /operational /operational
COPY --from=operational-build /build/application/target/release/truenas-exporter /truenas-exporter

# Test-only probe is exported to the host and mounted during validation.
FROM build AS probe-build
COPY deployment/tests/runtime_probe.rs /runtime_probe.rs
RUN rustc --edition 2024 -D warnings /runtime_probe.rs -o /runtime-probe

FROM scratch AS validation-probe
COPY --from=probe-build /runtime-probe /runtime-probe

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2 AS runtime
LABEL org.opencontainers.image.licenses="MIT"
COPY --from=build /build/application/target/release/truenas-exporter /usr/local/bin/truenas-exporter
COPY LICENSE /usr/share/doc/truenas-exporter/LICENSE
USER 10001:10001
EXPOSE 9110
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/truenas-exporter", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/truenas-exporter"]
