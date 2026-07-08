#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/version.sh"
OUTPUT_DIR="${1:-${ROOT_DIR}/dist/fn-knock-runtime}"
FORCE_FRONTEND_REBUILD="${FN_KNOCK_FORCE_FRONTEND_REBUILD:-1}"
BACKEND_IMPL="${FN_KNOCK_BACKEND_IMPL:-rust}"
BUILD_RUST_BACKEND="${FN_KNOCK_BUILD_RUST_BACKEND:-1}"
GATEWAY_ARCHES_RAW="${FN_KNOCK_RUNTIME_GATEWAY_ARCHES:-${FN_KNOCK_FPK_ARCHES:-amd64 arm64}}"
GATEWAY_ARCHES=()

ADMIN_DIST_DIR="${OUTPUT_DIR}/ui/www"
AUTH_DIST_DIR="${OUTPUT_DIR}/server-auth-view/dist"
SERVER_DIR="${OUTPUT_DIR}/server"
SERVER_ADMIN_DIR="${SERVER_DIR}/server-admin"
SERVER_ADMIN_RES_DIR="${SERVER_ADMIN_DIR}/resources"
SERVER_ADMIN_RS_BIN="${SERVER_DIR}/server-admin-rs"
ACME_RESOURCE_SRC="${ROOT_DIR}/apps/server-admin-rs/resources/acmesh.zip"

case "${BACKEND_IMPL}" in
  rust) ;;
  *) echo "[fn-knock] Invalid FN_KNOCK_BACKEND_IMPL=${BACKEND_IMPL}; Rust is the only runtime backend" >&2; exit 1 ;;
esac

read_gateway_arches() {
  local raw="${GATEWAY_ARCHES_RAW//,/ }"
  local arch
  local normalized
  local seen=" "

  for arch in ${raw}; do
    case "${arch}" in
      amd64|x86|x86_64)
        normalized="amd64"
        ;;
      arm64|aarch64)
        normalized="arm64"
        ;;
      arm32|armv8l|armv7|armv7l|armhf|arm)
        normalized="arm"
        ;;
      *)
        echo "[fn-knock] Invalid gateway architecture: ${arch}; expected amd64/x86, arm64, or arm" >&2
        exit 1
        ;;
    esac

    case "${seen}" in
      *" ${normalized} "*) ;;
      *)
        GATEWAY_ARCHES+=("${normalized}")
        seen="${seen}${normalized} "
        ;;
    esac
  done

  if [ "${#GATEWAY_ARCHES[@]}" -eq 0 ]; then
    echo "[fn-knock] Gateway architecture list is empty" >&2
    exit 1
  fi
}

read_gateway_arches

echo "[fn-knock] Assembling runtime into ${OUTPUT_DIR}"
echo "[fn-knock] Gateway architectures: ${GATEWAY_ARCHES[*]}"

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

if [ "${BUILD_RUST_BACKEND}" = "1" ]; then
  fn_knock_sync_rust_package_version "${ROOT_DIR}" "[fn-knock]"
  echo "[fn-knock] Building server-admin-rs..."
  cargo build --release --manifest-path "${ROOT_DIR}/apps/server-admin-rs/Cargo.toml"
fi

echo "[fn-knock] Preparing runtime directories..."
rm -rf "${SERVER_ADMIN_DIR}"
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

mkdir -p "${SERVER_ADMIN_RES_DIR}"

if [ "${BUILD_RUST_BACKEND}" = "1" ]; then
  echo "[fn-knock] Copying server-admin-rs binary"
  cp "${ROOT_DIR}/apps/server-admin-rs/target/release/server-admin-rs" "${SERVER_ADMIN_RS_BIN}"
  chmod 755 "${SERVER_ADMIN_RS_BIN}"
else
  rm -f "${SERVER_ADMIN_RS_BIN}"
fi

if [ ! -f "${ACME_RESOURCE_SRC}" ]; then
  echo "[fn-knock] Missing acme resource: ${ACME_RESOURCE_SRC}" >&2
  exit 1
fi

echo "[fn-knock] Copying bundled acme resource"
cp "${ACME_RESOURCE_SRC}" "${SERVER_ADMIN_RES_DIR}/acmesh.zip"

rm -f "${SERVER_DIR}"/go-reauth-proxy-linux-*
bash "${ROOT_DIR}/scripts/prepare-go-reauth-proxy.sh" "${SERVER_DIR}" "${GATEWAY_ARCHES[@]}"

echo "[fn-knock] Runtime assembly completed"
