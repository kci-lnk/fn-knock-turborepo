# syntax=docker/dockerfile:1.7

FROM --platform=$BUILDPLATFORM node:20-bookworm-slim AS base

WORKDIR /workspace

FROM base AS deps

COPY package.json package-lock.json bun.lock turbo.json ./
COPY apps/server-admin-rs/package.json ./apps/server-admin-rs/package.json
COPY apps/server-admin-view/package.json ./apps/server-admin-view/package.json
COPY apps/server-auth-view/package.json ./apps/server-auth-view/package.json
COPY packages/admin-shared/package.json ./packages/admin-shared/package.json
COPY packages/eslint-config/package.json ./packages/eslint-config/package.json
COPY packages/frontend-core/package.json ./packages/frontend-core/package.json
COPY packages/i18n/package.json ./packages/i18n/package.json
COPY packages/typescript-config/package.json ./packages/typescript-config/package.json
COPY packages/ui-vue/package.json ./packages/ui-vue/package.json

RUN --mount=type=cache,target=/root/.npm npm ci

FROM base AS admin-view-builder

COPY --from=deps /workspace/node_modules ./node_modules
COPY package.json package-lock.json bun.lock turbo.json ./
COPY apps/server-admin-view ./apps/server-admin-view
COPY packages ./packages

RUN npm run build --workspace server-admin-view

FROM base AS auth-view-builder

COPY --from=deps /workspace/node_modules ./node_modules
COPY package.json package-lock.json bun.lock turbo.json ./
COPY apps/server-auth-view ./apps/server-auth-view
COPY packages ./packages

RUN npm run build --workspace server-auth-view

FROM --platform=$TARGETPLATFORM debian:bookworm-slim AS runtime

ARG TARGETARCH
ARG TARGETVARIANT

ENV TZ=Asia/Shanghai \
  FN_KNOCK_BACKEND_IMPL=rust \
  FN_KNOCK_RUNTIME_TARGET=docker \
  FN_KNOCK_DATA_DIR=/var/lib/fn-knock \
  FN_KNOCK_GATEWAY_CONFIG_DIR=/usr/local/etc/fn-knock

WORKDIR /opt/fn-knock

RUN apt-get update \
  && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends curl ca-certificates openssl unzip tzdata \
  && ln -snf "/usr/share/zoneinfo/${TZ}" /etc/localtime \
  && echo "${TZ}" > /etc/timezone \
  && rm -rf /var/lib/apt/lists/*

COPY --from=admin-view-builder /workspace/apps/server-admin-view/dist/ /opt/fn-knock/ui/www/
COPY --from=auth-view-builder /workspace/apps/server-auth-view/dist/ /opt/fn-knock/server-auth-view/dist/
COPY apps/server-admin/resources/acmesh.zip /opt/fn-knock/server/server-admin/resources/acmesh.zip
COPY deploy/docker/rust-backends/ /opt/fn-knock/rust-backends/
COPY apps/fn-knock/app/server/go-reauth-proxy-linux-amd64 /opt/fn-knock/server/go-reauth-proxy-linux-amd64
COPY apps/fn-knock/app/server/go-reauth-proxy-linux-arm64 /opt/fn-knock/server/go-reauth-proxy-linux-arm64
COPY apps/fn-knock/app/server/go-reauth-proxy-linux-arm /opt/fn-knock/server/go-reauth-proxy-linux-arm
COPY deploy/docker/entrypoint.sh /usr/local/bin/fn-knock-entrypoint
COPY deploy/docker/reset-panel-password.sh /usr/local/bin/fn-knock-reset-panel-password

RUN chmod +x /usr/local/bin/fn-knock-entrypoint \
  && chmod +x /usr/local/bin/fn-knock-reset-panel-password \
  && chmod +x /opt/fn-knock/server/go-reauth-proxy-linux-amd64 /opt/fn-knock/server/go-reauth-proxy-linux-arm64 /opt/fn-knock/server/go-reauth-proxy-linux-arm \
  && case "${TARGETARCH}${TARGETVARIANT}" in \
    amd64*) gateway_arch="amd64" ;; \
    arm64*) gateway_arch="arm64" ;; \
    armv7|arm*) gateway_arch="arm" ;; \
    *) echo "unsupported Docker target architecture: ${TARGETARCH}${TARGETVARIANT}" >&2; exit 1 ;; \
  esac \
  && install -D "/opt/fn-knock/rust-backends/server-admin-rs-linux-${gateway_arch}" /opt/fn-knock/bin/server-admin-rs \
  && install -D "/opt/fn-knock/server/go-reauth-proxy-linux-${gateway_arch}" /opt/fn-knock/bin/go-reauth-proxy \
  && rm -rf /opt/fn-knock/rust-backends \
  && chmod +x /opt/fn-knock/bin/server-admin-rs \
  && chmod +x /opt/fn-knock/bin/go-reauth-proxy

EXPOSE 7991 7999

VOLUME ["/var/lib/fn-knock", "/usr/local/etc/fn-knock"]

ENTRYPOINT ["/usr/local/bin/fn-knock-entrypoint"]
