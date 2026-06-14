# syntax=docker/dockerfile:1.7

FROM node:20-bookworm-slim AS base

WORKDIR /workspace

FROM base AS deps

COPY package.json bun.lock turbo.json ./
COPY apps/server-admin/package.json ./apps/server-admin/package.json
COPY apps/server-admin-view/package.json ./apps/server-admin-view/package.json
COPY apps/server-auth-view/package.json ./apps/server-auth-view/package.json
COPY packages/admin-shared/package.json ./packages/admin-shared/package.json
COPY packages/eslint-config/package.json ./packages/eslint-config/package.json
COPY packages/frontend-core/package.json ./packages/frontend-core/package.json
COPY packages/i18n/package.json ./packages/i18n/package.json
COPY packages/typescript-config/package.json ./packages/typescript-config/package.json
COPY packages/ui-vue/package.json ./packages/ui-vue/package.json

RUN --mount=type=cache,target=/root/.npm npm install

FROM base AS admin-view-builder

COPY --from=deps /workspace/node_modules ./node_modules
COPY package.json bun.lock turbo.json ./
COPY apps/server-admin-view ./apps/server-admin-view
COPY packages ./packages

RUN npm run build --workspace server-admin-view

FROM base AS auth-view-builder

COPY --from=deps /workspace/node_modules ./node_modules
COPY package.json bun.lock turbo.json ./
COPY apps/server-auth-view ./apps/server-auth-view
COPY packages ./packages

RUN npm run build --workspace server-auth-view

FROM base AS server-builder

COPY --from=deps /workspace/node_modules ./node_modules
COPY package.json bun.lock turbo.json ./
COPY apps/server-admin ./apps/server-admin
COPY packages ./packages

RUN npm run build --workspace server-admin

FROM node:20-bookworm-slim AS runtime

ARG TARGETARCH

ENV NODE_ENV=production \
  TZ=Asia/Shanghai \
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
COPY --from=server-builder /workspace/apps/server-admin/dist/ /opt/fn-knock/server/server-admin/
COPY --from=server-builder /workspace/apps/server-admin/resources/acmesh.zip /opt/fn-knock/server/server-admin/resources/acmesh.zip
COPY apps/fn-knock/app/server/go-reauth-proxy-linux-amd64 /opt/fn-knock/server/go-reauth-proxy-linux-amd64
COPY apps/fn-knock/app/server/go-reauth-proxy-linux-arm64 /opt/fn-knock/server/go-reauth-proxy-linux-arm64
COPY apps/fn-knock/app/server/go-reauth-proxy-linux-arm /opt/fn-knock/server/go-reauth-proxy-linux-arm
COPY deploy/docker/entrypoint.sh /usr/local/bin/fn-knock-entrypoint
COPY deploy/docker/reset-panel-password.sh /usr/local/bin/fn-knock-reset-panel-password

RUN chmod +x /usr/local/bin/fn-knock-entrypoint \
  && chmod +x /usr/local/bin/fn-knock-reset-panel-password \
  && chmod +x /opt/fn-knock/server/go-reauth-proxy-linux-amd64 /opt/fn-knock/server/go-reauth-proxy-linux-arm64 /opt/fn-knock/server/go-reauth-proxy-linux-arm \
  && install -D "/opt/fn-knock/server/go-reauth-proxy-linux-${TARGETARCH}" /opt/fn-knock/bin/go-reauth-proxy \
  && chmod +x /opt/fn-knock/bin/go-reauth-proxy

EXPOSE 7991 7999

VOLUME ["/var/lib/fn-knock", "/usr/local/etc/fn-knock"]

ENTRYPOINT ["/usr/local/bin/fn-knock-entrypoint"]
