# syntax=docker/dockerfile:1.7

FROM --platform=$BUILDPLATFORM alpine:3.23 AS runtime-binaries

ARG TARGETARCH
ARG TARGETVARIANT

RUN --mount=type=bind,source=deploy/docker/rust-backends,target=/input/rust-backends,readonly \
  --mount=type=bind,source=apps/fn-knock/app/server,target=/input/server,readonly \
  case "${TARGETARCH}${TARGETVARIANT}" in \
    amd64*) gateway_arch="amd64" ;; \
    arm64*) gateway_arch="arm64" ;; \
    armv7|arm*) gateway_arch="arm" ;; \
    *) echo "unsupported Docker target architecture: ${TARGETARCH}${TARGETVARIANT}" >&2; exit 1 ;; \
  esac \
  && mkdir -p /runtime \
  && cp "/input/rust-backends/server-admin-rs-linux-${gateway_arch}" /runtime/server-admin-rs \
  && cp "/input/server/go-reauth-proxy-linux-${gateway_arch}" /runtime/go-reauth-proxy \
  && chmod +x /runtime/server-admin-rs /runtime/go-reauth-proxy

FROM --platform=$TARGETPLATFORM alpine:3.23 AS runtime

ENV TZ=Asia/Shanghai \
  FN_KNOCK_BACKEND_IMPL=rust \
  FN_KNOCK_RUNTIME_TARGET=docker \
  FN_KNOCK_DATA_DIR=/var/lib/fn-knock \
  FN_KNOCK_GATEWAY_CONFIG_DIR=/usr/local/etc/fn-knock

WORKDIR /opt/fn-knock

RUN apk add --no-cache bash ca-certificates curl openssl tzdata unzip \
  && ln -snf "/usr/share/zoneinfo/${TZ}" /etc/localtime \
  && echo "${TZ}" > /etc/timezone
COPY apps/fn-knock/app/ui/www/ /opt/fn-knock/ui/www/
COPY apps/fn-knock/app/server-auth-view/dist/ /opt/fn-knock/server-auth-view/dist/
COPY apps/server-admin-rs/resources/acmesh.zip /opt/fn-knock/server/server-admin/resources/acmesh.zip
COPY --from=runtime-binaries --chmod=755 /runtime/server-admin-rs /opt/fn-knock/bin/server-admin-rs
COPY --from=runtime-binaries --chmod=755 /runtime/go-reauth-proxy /opt/fn-knock/bin/go-reauth-proxy
COPY --chmod=755 deploy/docker/entrypoint.sh /usr/local/bin/fn-knock-entrypoint
COPY --chmod=755 deploy/docker/reset-panel-password.sh /usr/local/bin/fn-knock-reset-panel-password

EXPOSE 7991 7999

VOLUME ["/var/lib/fn-knock", "/usr/local/etc/fn-knock"]

ENTRYPOINT ["/usr/local/bin/fn-knock-entrypoint"]
