#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-${ROOT_DIR}/dist/fn-knock-runtime}"
FORCE_FRONTEND_REBUILD="${FN_KNOCK_FORCE_FRONTEND_REBUILD:-1}"

ADMIN_DIST_DIR="${OUTPUT_DIR}/ui/www"
AUTH_DIST_DIR="${OUTPUT_DIR}/server-auth-view/dist"
SERVER_DIR="${OUTPUT_DIR}/server"
SERVER_ADMIN_DIR="${SERVER_DIR}/server-admin"
SERVER_ADMIN_RES_DIR="${SERVER_ADMIN_DIR}/resources"
ACME_RESOURCE_SRC="${ROOT_DIR}/apps/server-admin/resources/acmesh.zip"

echo "[fn-knock] Assembling runtime into ${OUTPUT_DIR}"

cd "${ROOT_DIR}"

turbo_build_args=(
  run
  build
  --filter=server-admin-view
  --filter=server-auth-view
)

if [ "${FORCE_FRONTEND_REBUILD}" = "1" ]; then
  echo "[fn-knock] Building frontend apps (forced rebuild enabled)..."
  turbo_build_args+=(--force)
else
  echo "[fn-knock] Building frontend apps (allowing Turbo cache reuse)..."
fi

npx turbo "${turbo_build_args[@]}"

echo "[fn-knock] Building server-admin..."
npm run build --workspace server-admin

echo "[fn-knock] Preparing runtime directories..."
mkdir -p \
  "${ADMIN_DIST_DIR}" \
  "${AUTH_DIST_DIR}" \
  "${SERVER_ADMIN_DIR}" \
  "${SERVER_ADMIN_RES_DIR}" \
  "${SERVER_DIR}"

echo "[fn-knock] Syncing server-admin-view dist"
rsync -a --delete "${ROOT_DIR}/apps/server-admin-view/dist/" "${ADMIN_DIST_DIR}/"

echo "[fn-knock] Syncing server-auth-view dist"
rsync -a --delete "${ROOT_DIR}/apps/server-auth-view/dist/" "${AUTH_DIST_DIR}/"

echo "[fn-knock] Syncing server-admin dist"
rsync -a --delete "${ROOT_DIR}/apps/server-admin/dist/" "${SERVER_ADMIN_DIR}/"
mkdir -p "${SERVER_ADMIN_RES_DIR}"
cat > "${SERVER_ADMIN_DIR}/package.json" <<'EOF'
{
  "type": "module"
}
EOF

if [ ! -f "${ACME_RESOURCE_SRC}" ]; then
  echo "[fn-knock] Missing acme resource: ${ACME_RESOURCE_SRC}" >&2
  exit 1
fi

echo "[fn-knock] Copying bundled acme resource"
cp "${ACME_RESOURCE_SRC}" "${SERVER_ADMIN_RES_DIR}/acmesh.zip"

rm -f "${SERVER_DIR}"/go-reauth-proxy-linux-*
bash "${ROOT_DIR}/scripts/prepare-go-reauth-proxy.sh" "${SERVER_DIR}" amd64 arm64

echo "[fn-knock] Runtime assembly completed"
