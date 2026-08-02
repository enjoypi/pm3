ARG BUILDER_IMAGE=docker.io/library/rust:1.95-trixie
ARG RUNTIME_IMAGE=docker.io/library/debian:trixie-slim

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
    cargo install --locked --path frameworks --target "${triple}" --root /out

FROM ${RUNTIME_IMAGE}

# bubblewrap 是 pm3 在 Linux 上的沙盒后端，缺失时 pm3 start 会 fail-closed 拒绝启动。
# 容器内 bwrap 需要 user namespace 权限：以 --cap-add SYS_ADMIN 或
# --security-opt seccomp=unconfined 运行，否则沙盒无法创建 namespace。
# procps 提供 /bin/ps，pm3 用它采集身份令牌判进程存活；缺失时每次 daemon 重启
# 都探不出结果，接管来的服务会被判为「探测失败」而驱逐重启。
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends bubblewrap procps; \
    rm -rf /var/lib/apt/lists/*; \
    useradd --create-home --uid 65532 --user-group nonroot

WORKDIR /

COPY --from=builder /out/bin/pm3 /usr/local/bin/pm3
COPY --from=builder --chown=nonroot:nonroot /app/config.yaml /config.yaml

ENV PM3_HOME=/home/nonroot/.pm3

USER nonroot

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD ["pgrep", "-x", "pm3"]

ENTRYPOINT ["/usr/local/bin/pm3", "--config", "/config.yaml"]
CMD ["daemon"]
