ARG BUILDER_IMAGE=docker.io/library/rust:1.95-trixie
ARG RUNTIME_IMAGE=gcr.io/distroless/cc-debian13:nonroot

FROM --platform=$BUILDPLATFORM ${BUILDER_IMAGE} AS builder

ARG TARGETARCH

WORKDIR /app
COPY . .

ENV CARGO_PROFILE_RELEASE_OPT_LEVEL=3 \
    CARGO_PROFILE_RELEASE_DEBUG=false \
    CARGO_PROFILE_RELEASE_STRIP=true \
    CARGO_TARGET_DIR=/app/target

RUN set -eux; \
    case "${TARGETARCH}" in \
      amd64) triple=x86_64-unknown-linux-gnu ;; \
      arm64) triple=aarch64-unknown-linux-gnu ;; \
      *) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    echo "${triple}" > /tmp/target-triple; \
    if [ "$(rustc -vV | sed -n 's|^host: ||p')" != "${triple}" ]; then \
      pkg_arch="$(echo "${triple}" | cut -d- -f1 | tr '_' '-')"; \
      apt-get update; \
      apt-get install -y --no-install-recommends "gcc-${pkg_arch}-linux-gnu"; \
      rm -rf /var/lib/apt/lists/*; \
    fi; \
    rustup target add "${triple}"

RUN --mount=type=cache,id=cargo-registry-${TARGETARCH},target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git-${TARGETARCH},target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target-${TARGETARCH},target=/app/target \
    set -eux; \
    triple="$(cat /tmp/target-triple)"; \
    if [ "$(rustc -vV | sed -n 's|^host: ||p')" != "${triple}" ]; then \
      cc="$(echo "${triple}" | cut -d- -f1)-linux-gnu-gcc"; \
      triple_snake="$(echo "${triple}" | tr '-' '_')"; \
      export "CARGO_TARGET_$(echo "${triple_snake}" | tr 'a-z' 'A-Z')_LINKER=${cc}"; \
      export "CC_${triple_snake}=${cc}"; \
    fi; \
    cargo install --locked --path frameworks --features http,sqlite --target "${triple}" --root /out

FROM ${RUNTIME_IMAGE}

WORKDIR /

COPY --from=builder --chown=nonroot:nonroot /out/bin/* /entrypoint
COPY --from=builder --chown=nonroot:nonroot /app/config.yaml /config.yaml
COPY --from=builder --chown=nonroot:nonroot /app/migrations /migrations

ENV DATABASE_URL=sqlite:///home/nonroot/data.db?mode=rwc

EXPOSE 9229

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD ["/entrypoint", "--config", "/config.yaml", "health-check"]

ENTRYPOINT ["/entrypoint", "--config", "/config.yaml"]
CMD ["serve"]
