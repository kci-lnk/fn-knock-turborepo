#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOCKER_DIR="${ROOT_DIR}/deploy/docker"
LOCAL_COMPOSE_FILE="${DOCKER_DIR}/compose.yaml"
LOCAL_OVERRIDE_FILE="${DOCKER_DIR}/compose.override.yaml"
REMOTE_COMPOSE_TEMPLATE="${DOCKER_DIR}/compose.remote.yaml"
DEFAULT_ENV_FILE="${DOCKER_DIR}/.env"
FALLBACK_ENV_FILE="${DOCKER_DIR}/.env.example"
VERSION_FILE="${ROOT_DIR}/apps/server-admin/src/lib/app-version.ts"

REMOTE_HOST="${FN_KNOCK_DOCKER_REMOTE_HOST:-root@192.168.31.135}"
REMOTE_DIR="${FN_KNOCK_DOCKER_REMOTE_DIR:-/opt/fn-knock-docker}"
REMOTE_COMPOSE_PATH="${REMOTE_DIR}/compose.yaml"
REMOTE_ENV_PATH="${REMOTE_DIR}/.env"
SERVICE_NAME="${FN_KNOCK_DOCKER_SERVICE_NAME:-fn-knock}"
WAIT_TIMEOUT="${FN_KNOCK_DOCKER_WAIT_TIMEOUT:-180}"
CACHE_ROOT="${FN_KNOCK_DOCKER_CACHE_DIR:-${HOME}/.cache/fn-knock-buildx}"
BUILDER_NAME="${FN_KNOCK_DOCKER_BUILDER:-}"
MANAGED_BUILDER_NAME="${FN_KNOCK_DOCKER_MANAGED_BUILDER:-fn-knock-buildx}"
PROXY_HOST_ALIAS="${FN_KNOCK_DOCKER_PROXY_HOST_ALIAS:-host.docker.internal}"

TEMP_FILES=()
EFFECTIVE_BUILDER_NAME=""
EFFECTIVE_BUILDER_DRIVER=""
BUILD_HTTP_PROXY=""
BUILD_HTTPS_PROXY=""
BUILD_ALL_PROXY=""
BUILD_NO_PROXY=""
BUILD_PROXY_ENABLED=0
DOCKER_ARCHES=(amd64 arm64 arm32)
GATEWAY_BINARY_DIR="${ROOT_DIR}/apps/fn-knock/app/server"
GATEWAY_BINARIES_PREPARED=0

cleanup_temp_files() {
  if [ "${#TEMP_FILES[@]}" -gt 0 ]; then
    rm -f "${TEMP_FILES[@]}"
  fi
}

trap cleanup_temp_files EXIT

log() {
  echo "[fn-knock-docker] $*"
}

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  command -v "${cmd}" >/dev/null 2>&1 || fail "missing required command: ${cmd}"
}

prepare_gateway_binaries() {
  if [ "${GATEWAY_BINARIES_PREPARED}" = "1" ]; then
    return 0
  fi

  bash "${ROOT_DIR}/scripts/prepare-go-reauth-proxy.sh" "${GATEWAY_BINARY_DIR}" amd64 arm64 arm
  GATEWAY_BINARIES_PREPARED=1
}

read_proxy_value() {
  local explicit_var="$1"
  local upper_var="$2"
  local lower_var="$3"
  local value="${!explicit_var:-}"

  if [ -n "${value}" ]; then
    printf '%s' "${value}"
    return 0
  fi

  value="${!upper_var:-}"
  if [ -n "${value}" ]; then
    printf '%s' "${value}"
    return 0
  fi

  value="${!lower_var:-}"
  printf '%s' "${value}"
}

normalize_proxy_for_container() {
  local value="$1"

  if [ -z "${value}" ]; then
    printf '%s' ""
    return 0
  fi

  printf '%s' "${value}" | sed -E \
    -e "s#://(([^/@]+@)?)(127\\.0\\.0\\.1|localhost)([:/]|$)#://\\1${PROXY_HOST_ALIAS}\\4#" \
    -e "s#^(([^/@]+@)?)(127\\.0\\.0\\.1|localhost)([:/]|$)#\\1${PROXY_HOST_ALIAS}\\4#" \
    -e "s#://(([^/@]+@)?)\\[::1\\]([:/]|$)#://\\1${PROXY_HOST_ALIAS}\\3#" \
    -e "s#^(([^/@]+@)?)\\[::1\\]([:/]|$)#\\1${PROXY_HOST_ALIAS}\\3#"
}

configure_build_proxy() {
  local raw_http_proxy
  local raw_https_proxy
  local raw_all_proxy
  local raw_no_proxy

  raw_http_proxy="$(read_proxy_value "FN_KNOCK_DOCKER_HTTP_PROXY" "HTTP_PROXY" "http_proxy")"
  raw_https_proxy="$(read_proxy_value "FN_KNOCK_DOCKER_HTTPS_PROXY" "HTTPS_PROXY" "https_proxy")"
  raw_all_proxy="$(read_proxy_value "FN_KNOCK_DOCKER_ALL_PROXY" "ALL_PROXY" "all_proxy")"
  raw_no_proxy="$(read_proxy_value "FN_KNOCK_DOCKER_NO_PROXY" "NO_PROXY" "no_proxy")"

  if [ -z "${raw_https_proxy}" ] && [ -n "${raw_http_proxy}" ]; then
    raw_https_proxy="${raw_http_proxy}"
  fi

  BUILD_HTTP_PROXY="$(normalize_proxy_for_container "${raw_http_proxy}")"
  BUILD_HTTPS_PROXY="$(normalize_proxy_for_container "${raw_https_proxy}")"
  BUILD_ALL_PROXY="$(normalize_proxy_for_container "${raw_all_proxy}")"
  BUILD_NO_PROXY="$(normalize_proxy_for_container "${raw_no_proxy}")"
  BUILD_PROXY_ENABLED=0

  if [ -n "${BUILD_HTTP_PROXY}${BUILD_HTTPS_PROXY}${BUILD_ALL_PROXY}${BUILD_NO_PROXY}" ]; then
    BUILD_PROXY_ENABLED=1
  fi
}

buildkit_container_name() {
  local builder_name="$1"
  printf 'buildx_buildkit_%s0' "${builder_name}"
}

builder_proxy_env_missing() {
  local builder_name="$1"
  local env_output
  local container_name

  [ "${BUILD_PROXY_ENABLED}" = "1" ] || return 1

  container_name="$(buildkit_container_name "${builder_name}")"
  env_output="$(docker inspect "${container_name}" --format '{{range .Config.Env}}{{println .}}{{end}}' 2>/dev/null || true)"

  if [ -z "${env_output}" ]; then
    return 0
  fi

  if [ -n "${BUILD_HTTP_PROXY}" ] && ! printf '%s\n' "${env_output}" | grep -Fxq "http_proxy=${BUILD_HTTP_PROXY}"; then
    return 0
  fi
  if [ -n "${BUILD_HTTP_PROXY}" ] && ! printf '%s\n' "${env_output}" | grep -Fxq "HTTP_PROXY=${BUILD_HTTP_PROXY}"; then
    return 0
  fi
  if [ -n "${BUILD_HTTPS_PROXY}" ] && ! printf '%s\n' "${env_output}" | grep -Fxq "https_proxy=${BUILD_HTTPS_PROXY}"; then
    return 0
  fi
  if [ -n "${BUILD_HTTPS_PROXY}" ] && ! printf '%s\n' "${env_output}" | grep -Fxq "HTTPS_PROXY=${BUILD_HTTPS_PROXY}"; then
    return 0
  fi
  if [ -n "${BUILD_ALL_PROXY}" ] && ! printf '%s\n' "${env_output}" | grep -Fxq "all_proxy=${BUILD_ALL_PROXY}"; then
    return 0
  fi
  if [ -n "${BUILD_ALL_PROXY}" ] && ! printf '%s\n' "${env_output}" | grep -Fxq "ALL_PROXY=${BUILD_ALL_PROXY}"; then
    return 0
  fi
  if [ -n "${BUILD_NO_PROXY}" ] && ! printf '%s\n' "${env_output}" | grep -Fxq "no_proxy=${BUILD_NO_PROXY}"; then
    return 0
  fi
  if [ -n "${BUILD_NO_PROXY}" ] && ! printf '%s\n' "${env_output}" | grep -Fxq "NO_PROXY=${BUILD_NO_PROXY}"; then
    return 0
  fi

  return 1
}

resolve_env_file() {
  if [ -n "${FN_KNOCK_DOCKER_ENV_FILE:-}" ]; then
    echo "${FN_KNOCK_DOCKER_ENV_FILE}"
    return 0
  fi

  if [ -f "${DEFAULT_ENV_FILE}" ]; then
    echo "${DEFAULT_ENV_FILE}"
    return 0
  fi

  echo "${FALLBACK_ENV_FILE}"
}

ENV_FILE="$(resolve_env_file)"

require_env_file() {
  [ -f "${ENV_FILE}" ] || fail "env file not found: ${ENV_FILE}"
}

read_env_value() {
  local key="$1"
  local default_value="${2:-}"
  local value

  value="$(
    awk -v key="${key}" '
      /^[[:space:]]*#/ { next }
      index($0, "=") == 0 { next }
      {
        current_key = substr($0, 1, index($0, "=") - 1)
        if (current_key == key) {
          print substr($0, index($0, "=") + 1)
          exit
        }
      }
    ' "${ENV_FILE}" 2>/dev/null || true
  )"

  if [ -n "${value}" ]; then
    echo "${value}"
    return 0
  fi

  echo "${default_value}"
}

resolve_local_image() {
  if [ -n "${FN_KNOCK_DOCKER_IMAGE:-}" ]; then
    echo "${FN_KNOCK_DOCKER_IMAGE}"
    return 0
  fi

  read_env_value "FN_KNOCK_IMAGE" "fn-knock:local"
}

parse_app_version() {
  local version

  [ -f "${VERSION_FILE}" ] || fail "missing version file: ${VERSION_FILE}"
  version="$(sed -nE 's/^[[:space:]]*export[[:space:]]+const[[:space:]]+APP_LOCAL_VERSION[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "${VERSION_FILE}" | head -n1)"
  [ -n "${version}" ] || fail "failed to parse APP_LOCAL_VERSION from ${VERSION_FILE}"

  echo "${version}"
}

normalize_arch() {
  case "$1" in
    amd64 | x86_64)
      echo "amd64"
      ;;
    arm64 | aarch64)
      echo "arm64"
      ;;
    arm32 | armv8l | armv7 | armv7l | armhf | arm)
      echo "arm32"
      ;;
    *)
      fail "unsupported architecture: $1"
      ;;
  esac
}

docker_platform_for_arch() {
  case "$1" in
    amd64)
      echo "linux/amd64"
      ;;
    arm64)
      echo "linux/arm64"
      ;;
    arm32)
      echo "linux/arm/v7"
      ;;
    *)
      fail "unsupported docker architecture: $1"
      ;;
  esac
}

detect_remote_arch() {
  local raw_arch
  raw_arch="$(ssh "${REMOTE_HOST}" "uname -m")"
  normalize_arch "${raw_arch}"
}

detect_local_arch() {
  if [ -n "${FN_KNOCK_DOCKER_LOCAL_ARCH:-}" ]; then
    normalize_arch "${FN_KNOCK_DOCKER_LOCAL_ARCH}"
    return 0
  fi

  normalize_arch "$(uname -m)"
}

build_default_remote_tag_base() {
  local version
  local timestamp

  version="$(parse_app_version)"
  timestamp="$(date +%Y%m%d%H%M%S)"

  echo "${version}-${timestamp}"
}

build_default_publish_tag_base() {
  parse_app_version
}

normalize_tag_base() {
  local tag_base="$1"

  case "${tag_base}" in
    *-amd64)
      echo "${tag_base%-amd64}"
      ;;
    *-arm64)
      echo "${tag_base%-arm64}"
      ;;
    *-arm32)
      echo "${tag_base%-arm32}"
      ;;
    *)
      echo "${tag_base}"
      ;;
  esac
}

build_arch_image_ref() {
  local image_repo="$1"
  local tag_base="$2"
  local arch="$3"

  echo "${image_repo}:${tag_base}-${arch}"
}

require_publish_image_repo() {
  local image_repo="$1"

  case "${image_repo}" in
    */*)
      return 0
      ;;
    *)
      fail "publish target must include namespace, for example FN_KNOCK_DOCKER_IMAGE_REPO=kcilnk/fn-knock"
      ;;
  esac
}

upsert_env_file() {
  local input_file="$1"
  local output_file="$2"
  local key="$3"
  local value="$4"

  awk -v key="${key}" -v value="${value}" '
    BEGIN { updated = 0 }
    $0 ~ ("^[[:space:]]*" key "=") {
      print key "=" value
      updated = 1
      next
    }
    { print }
    END {
      if (!updated) {
        print key "=" value
      }
    }
  ' "${input_file}" > "${output_file}"
}

prepare_remote_env_file() {
  local image_ref="$1"
  local temp_env

  temp_env="$(mktemp)"
  TEMP_FILES+=("${temp_env}")
  upsert_env_file "${ENV_FILE}" "${temp_env}" "FN_KNOCK_IMAGE" "${image_ref}"

  echo "${temp_env}"
}

compose_local() {
  docker compose \
    --env-file "${ENV_FILE}" \
    -f "${LOCAL_COMPOSE_FILE}" \
    -f "${LOCAL_OVERRIDE_FILE}" \
    "$@"
}

run_remote_compose() {
  local escaped_args=()
  local arg

  for arg in "$@"; do
    escaped_args+=("$(printf "%q" "${arg}")")
  done

  ssh "${REMOTE_HOST}" \
    "cd $(printf "%q" "${REMOTE_DIR}") && docker compose --env-file $(printf "%q" "${REMOTE_ENV_PATH}") -f $(printf "%q" "${REMOTE_COMPOSE_PATH}") ${escaped_args[*]}"
}

ensure_remote_prerequisites() {
  ssh "${REMOTE_HOST}" "command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1" >/dev/null
}

ensure_buildx_builder() {
  local inspect_output
  local create_args=()

  configure_build_proxy

  if [ -n "${EFFECTIVE_BUILDER_NAME}" ] && [ -n "${EFFECTIVE_BUILDER_DRIVER}" ]; then
    return 0
  fi

  if [ -n "${BUILDER_NAME}" ]; then
    EFFECTIVE_BUILDER_NAME="${BUILDER_NAME}"
    docker buildx inspect "${EFFECTIVE_BUILDER_NAME}" >/dev/null 2>&1 || \
      fail "specified buildx builder not found: ${EFFECTIVE_BUILDER_NAME}"
    if builder_proxy_env_missing "${EFFECTIVE_BUILDER_NAME}"; then
      fail "specified buildx builder ${EFFECTIVE_BUILDER_NAME} is missing proxy env; recreate it with proxy support or unset FN_KNOCK_DOCKER_BUILDER"
    fi
  else
    EFFECTIVE_BUILDER_NAME="${MANAGED_BUILDER_NAME}"
    if docker buildx inspect "${EFFECTIVE_BUILDER_NAME}" >/dev/null 2>&1 && builder_proxy_env_missing "${EFFECTIVE_BUILDER_NAME}"; then
      log "Recreating managed buildx builder ${EFFECTIVE_BUILDER_NAME} to apply proxy settings"
      docker buildx rm "${EFFECTIVE_BUILDER_NAME}" >/dev/null
    fi
    if ! docker buildx inspect "${EFFECTIVE_BUILDER_NAME}" >/dev/null 2>&1; then
      log "Creating managed buildx builder ${EFFECTIVE_BUILDER_NAME} with docker-container driver"
      create_args=(--name "${EFFECTIVE_BUILDER_NAME}" --driver docker-container)
      if [ -n "${BUILD_HTTP_PROXY}" ]; then
        create_args+=(--driver-opt "env.http_proxy=${BUILD_HTTP_PROXY}" --driver-opt "env.HTTP_PROXY=${BUILD_HTTP_PROXY}")
      fi
      if [ -n "${BUILD_HTTPS_PROXY}" ]; then
        create_args+=(--driver-opt "env.https_proxy=${BUILD_HTTPS_PROXY}" --driver-opt "env.HTTPS_PROXY=${BUILD_HTTPS_PROXY}")
      fi
      if [ -n "${BUILD_ALL_PROXY}" ]; then
        create_args+=(--driver-opt "env.all_proxy=${BUILD_ALL_PROXY}" --driver-opt "env.ALL_PROXY=${BUILD_ALL_PROXY}")
      fi
      if [ -n "${BUILD_NO_PROXY}" ]; then
        create_args+=(--driver-opt "env.no_proxy=${BUILD_NO_PROXY}" --driver-opt "env.NO_PROXY=${BUILD_NO_PROXY}")
      fi
      docker buildx create "${create_args[@]}" >/dev/null
    fi
  fi

  inspect_output="$(docker buildx inspect --bootstrap "${EFFECTIVE_BUILDER_NAME}")" || \
    fail "failed to bootstrap buildx builder: ${EFFECTIVE_BUILDER_NAME}"
  EFFECTIVE_BUILDER_DRIVER="$(printf '%s\n' "${inspect_output}" | sed -n 's/^Driver:[[:space:]]*//p' | head -n1)"
  [ -n "${EFFECTIVE_BUILDER_DRIVER}" ] || fail "failed to detect buildx driver for ${EFFECTIVE_BUILDER_NAME}"

  log "Using buildx builder ${EFFECTIVE_BUILDER_NAME} (${EFFECTIVE_BUILDER_DRIVER})"
}

finalize_cache_dir() {
  local cache_dir="$1"
  local cache_next="$2"

  if [ -d "${cache_next}" ]; then
    rm -rf "${cache_dir}"
    mv "${cache_next}" "${cache_dir}"
  fi
}

run_buildx_image() {
  local arch="$1"
  local image_ref="$2"
  local output_mode="$3"
  local cache_scope="${4:-${arch}}"
  local cache_dir="${CACHE_ROOT}/${cache_scope}"
  local cache_next="${cache_dir}-next"
  local platform
  local build_args=()
  local cache_export_enabled=1

  platform="$(docker_platform_for_arch "${arch}")"
  prepare_gateway_binaries
  configure_build_proxy
  log "Building image ${image_ref} for ${platform} (${output_mode})"
  ensure_buildx_builder

  mkdir -p "${CACHE_ROOT}"
  rm -rf "${cache_next}"
  build_args+=(--builder "${EFFECTIVE_BUILDER_NAME}")

  if [ "${BUILD_PROXY_ENABLED}" = "1" ]; then
    log "Docker build proxy enabled via ${PROXY_HOST_ALIAS}"
    if [ -n "${BUILD_HTTP_PROXY}" ]; then
      build_args+=(--build-arg "HTTP_PROXY=${BUILD_HTTP_PROXY}" --build-arg "http_proxy=${BUILD_HTTP_PROXY}")
    fi
    if [ -n "${BUILD_HTTPS_PROXY}" ]; then
      build_args+=(--build-arg "HTTPS_PROXY=${BUILD_HTTPS_PROXY}" --build-arg "https_proxy=${BUILD_HTTPS_PROXY}")
    fi
    if [ -n "${BUILD_ALL_PROXY}" ]; then
      build_args+=(--build-arg "ALL_PROXY=${BUILD_ALL_PROXY}" --build-arg "all_proxy=${BUILD_ALL_PROXY}")
    fi
    if [ -n "${BUILD_NO_PROXY}" ]; then
      build_args+=(--build-arg "NO_PROXY=${BUILD_NO_PROXY}" --build-arg "no_proxy=${BUILD_NO_PROXY}")
    fi
  fi

  if [ "${EFFECTIVE_BUILDER_DRIVER}" = "docker" ]; then
    cache_export_enabled=0
    log "Builder ${EFFECTIVE_BUILDER_NAME} uses docker driver; skipping local cache export"
  fi

  if [ "${cache_export_enabled}" = "1" ] && [ -d "${cache_dir}" ]; then
    build_args+=(--cache-from "type=local,src=${cache_dir}")
  fi

  case "${output_mode}" in
    load)
      build_args+=(--load)
      ;;
    push)
      build_args+=(--push)
      ;;
    *)
      fail "unsupported buildx output mode: ${output_mode}"
      ;;
  esac

  build_args+=(
    --platform "${platform}" \
    -f "${DOCKER_DIR}/Dockerfile" \
    -t "${image_ref}" \
    .
  )

  if [ "${cache_export_enabled}" = "1" ]; then
    build_args=(--cache-to "type=local,dest=${cache_next},mode=max" "${build_args[@]}")
  fi

  docker buildx build "${build_args[@]}"
  if [ "${cache_export_enabled}" = "1" ]; then
    finalize_cache_dir "${cache_dir}" "${cache_next}"
  fi
}

buildx_image() {
  run_buildx_image "$1" "$2" load "$1"
}

pushx_image() {
  run_buildx_image "$1" "$2" push "$1"
}

build_local_image() {
  local image_ref="$1"
  local arch

  arch="$(detect_local_arch)"
  buildx_image "${arch}" "${image_ref}"
}

build_remote_image() {
  local arch="$1"
  local image_ref="$2"

  buildx_image "${arch}" "${image_ref}"
}

stream_image_to_remote() {
  local image_ref="$1"

  log "Streaming image ${image_ref} to ${REMOTE_HOST}"
  docker save "${image_ref}" | ssh "${REMOTE_HOST}" "docker load"
}

create_manifest_tag() {
  local target_ref="$1"
  shift

  log "Creating multi-arch manifest ${target_ref}"
  docker buildx imagetools create -t "${target_ref}" "$@"
}

resolve_manifest_digest() {
  local target_ref="$1"
  local inspect_output
  local digest

  inspect_output="$(docker buildx imagetools inspect "${target_ref}")" || \
    fail "failed to inspect manifest ${target_ref}"
  digest="$(printf '%s\n' "${inspect_output}" | awk '/^Digest:[[:space:]]*/ { print $2; exit }')"
  [ -n "${digest}" ] || fail "failed to resolve manifest digest for ${target_ref}"

  printf '%s\n' "${digest}"
}

wait_for_manifest_digest() {
  local target_ref="$1"
  local expected_digest="$2"
  local expected_label="${3:-${expected_digest}}"
  local max_attempts="${FN_KNOCK_DOCKER_MANIFEST_VERIFY_ATTEMPTS:-6}"
  local delay_seconds="${FN_KNOCK_DOCKER_MANIFEST_VERIFY_DELAY:-5}"
  local attempt=1
  local actual_digest=""

  while [ "${attempt}" -le "${max_attempts}" ]; do
    actual_digest="$(resolve_manifest_digest "${target_ref}")"
    if [ "${actual_digest}" = "${expected_digest}" ]; then
      log "Verified manifest ${target_ref} points at ${expected_label} (${expected_digest})"
      return 0
    fi

    if [ "${attempt}" -lt "${max_attempts}" ]; then
      log "Manifest ${target_ref} digest is ${actual_digest}; waiting for ${expected_label} (${expected_digest})"
      sleep "${delay_seconds}"
    fi

    attempt=$((attempt + 1))
  done

  fail "manifest ${target_ref} digest ${actual_digest} does not match ${expected_label} (${expected_digest})"
}

verify_manifest_platforms() {
  local target_ref="$1"
  local inspect_output

  inspect_output="$(docker buildx imagetools inspect "${target_ref}")" || \
    fail "failed to inspect manifest ${target_ref}"

  printf '%s\n' "${inspect_output}" | grep -q 'linux/amd64' || \
    fail "manifest ${target_ref} is missing linux/amd64"
  printf '%s\n' "${inspect_output}" | grep -q 'linux/arm64' || \
    fail "manifest ${target_ref} is missing linux/arm64"
  printf '%s\n' "${inspect_output}" | grep -q 'linux/arm/v7' || \
    fail "manifest ${target_ref} is missing linux/arm/v7"

  log "Verified manifest ${target_ref} includes linux/amd64, linux/arm64, and linux/arm/v7"
}

upload_remote_bundle() {
  local temp_env="$1"

  log "Uploading compose bundle to ${REMOTE_HOST}:${REMOTE_DIR}"
  ssh "${REMOTE_HOST}" "mkdir -p '${REMOTE_DIR}'"
  scp "${REMOTE_COMPOSE_TEMPLATE}" "${REMOTE_HOST}:${REMOTE_COMPOSE_PATH}" >/dev/null
  scp "${temp_env}" "${REMOTE_HOST}:${REMOTE_ENV_PATH}" >/dev/null
}

show_remote_logs() {
  run_remote_compose logs --tail=200 "${SERVICE_NAME}" || true
}

wait_for_remote_health() {
  local start_ts
  local now_ts
  local container_id
  local health_state

  log "Waiting for remote service health"
  start_ts="$(date +%s)"

  while true; do
    container_id="$(run_remote_compose ps -q "${SERVICE_NAME}" | tr -d '\r' | tail -n1)"
    if [ -n "${container_id}" ]; then
      health_state="$(
        ssh "${REMOTE_HOST}" \
          "docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' '${container_id}' 2>/dev/null || true"
      )"

      case "${health_state}" in
        healthy | running)
          log "Remote service is ${health_state}"
          return 0
          ;;
        starting | created | restarting | "")
          ;;
        unhealthy | exited | dead)
          show_remote_logs
          fail "remote service entered bad state: ${health_state}"
          ;;
        *)
          log "Remote service state: ${health_state}"
          ;;
      esac
    fi

    now_ts="$(date +%s)"
    if [ $((now_ts - start_ts)) -ge "${WAIT_TIMEOUT}" ]; then
      show_remote_logs
      fail "timed out waiting for remote service health after ${WAIT_TIMEOUT}s"
    fi

    sleep 3
  done
}

print_remote_access_hint() {
  local remote_host_for_url
  local admin_view_port
  local proxy_port

  remote_host_for_url="${REMOTE_HOST##*@}"
  remote_host_for_url="${remote_host_for_url%%:*}"
  admin_view_port="$(read_env_value "ADMIN_VIEW_PORT" "7991")"
  proxy_port="$(read_env_value "GO_REPROXY_PORT" "7999")"

  log "Remote admin view URL: http://${remote_host_for_url}:${admin_view_port}"
  log "Remote admin backend port stays internal in Docker mode"
  log "Remote proxy port: ${proxy_port}"
}

cmd_build_local() {
  local image_ref

  require_cmd docker
  require_env_file

  log "Using env file ${ENV_FILE}"
  image_ref="$(resolve_local_image)"
  build_local_image "${image_ref}"
}

cmd_up_local() {
  require_cmd docker
  require_env_file

  log "Using env file ${ENV_FILE}"
  build_local_image "$(resolve_local_image)"
  compose_local up
}

cmd_down_local() {
  require_cmd docker
  require_env_file

  log "Using env file ${ENV_FILE}"
  compose_local down --remove-orphans
}

cmd_logs_local() {
  require_cmd docker
  require_env_file

  log "Using env file ${ENV_FILE}"
  compose_local logs --tail=200 -f "${SERVICE_NAME}"
}

cmd_reset_panel_password_local() {
  require_cmd docker
  require_env_file

  log "Using env file ${ENV_FILE}"
  compose_local exec -T "${SERVICE_NAME}" fn-knock-reset-panel-password
}

cmd_remote_ps() {
  require_cmd ssh
  ensure_remote_prerequisites
  run_remote_compose ps
}

cmd_remote_logs() {
  require_cmd ssh
  ensure_remote_prerequisites
  run_remote_compose logs --tail=200 -f "${SERVICE_NAME}"
}

cmd_reset_panel_password_remote() {
  require_cmd ssh
  ensure_remote_prerequisites
  run_remote_compose exec -T "${SERVICE_NAME}" fn-knock-reset-panel-password
}

cmd_local_deploy() {
  local remote_arch
  local image_repo
  local tag_base
  local runtime_image_ref
  local remote_env_file
  local arch
  local image_ref

  require_cmd docker
  require_cmd ssh
  require_cmd scp
  require_env_file
  ensure_remote_prerequisites

  remote_arch="$(detect_remote_arch)"
  image_repo="${FN_KNOCK_DOCKER_IMAGE_REPO:-fn-knock}"
  tag_base="${FN_KNOCK_DOCKER_IMAGE_TAG:-$(build_default_remote_tag_base)}"
  tag_base="$(normalize_tag_base "${tag_base}")"
  runtime_image_ref="$(build_arch_image_ref "${image_repo}" "${tag_base}" "${remote_arch}")"
  remote_env_file="$(prepare_remote_env_file "${runtime_image_ref}")"

  log "Using env file ${ENV_FILE}"
  log "Remote host ${REMOTE_HOST} detected as ${remote_arch}"
  log "Deploy image set ${image_repo}:${tag_base}-amd64, ${image_repo}:${tag_base}-arm64, and ${image_repo}:${tag_base}-arm32"
  log "Remote runtime image ${runtime_image_ref}"

  for arch in "${DOCKER_ARCHES[@]}"; do
    image_ref="$(build_arch_image_ref "${image_repo}" "${tag_base}" "${arch}")"
    build_remote_image "${arch}" "${image_ref}"
    stream_image_to_remote "${image_ref}"
  done

  upload_remote_bundle "${remote_env_file}"

  log "Restarting remote compose stack"
  if ! run_remote_compose up -d --remove-orphans --force-recreate; then
    show_remote_logs
    fail "remote docker compose up failed"
  fi

  wait_for_remote_health
  run_remote_compose ps
  print_remote_access_hint
}

cmd_local_deploy_fast() {
  local remote_arch
  local image_repo
  local tag_base
  local runtime_image_ref
  local remote_env_file

  require_cmd docker
  require_cmd ssh
  require_cmd scp
  require_env_file
  ensure_remote_prerequisites

  remote_arch="$(detect_remote_arch)"
  image_repo="${FN_KNOCK_DOCKER_IMAGE_REPO:-fn-knock}"
  tag_base="${FN_KNOCK_DOCKER_IMAGE_TAG:-$(build_default_remote_tag_base)}"
  tag_base="$(normalize_tag_base "${tag_base}")"
  runtime_image_ref="$(build_arch_image_ref "${image_repo}" "${tag_base}" "${remote_arch}")"
  remote_env_file="$(prepare_remote_env_file "${runtime_image_ref}")"

  log "Using env file ${ENV_FILE}"
  log "Remote host ${REMOTE_HOST} detected as ${remote_arch}"
  log "Fast deploy image ${runtime_image_ref}"

  build_remote_image "${remote_arch}" "${runtime_image_ref}"
  stream_image_to_remote "${runtime_image_ref}"

  upload_remote_bundle "${remote_env_file}"

  log "Restarting remote compose stack"
  if ! run_remote_compose up -d --remove-orphans --force-recreate; then
    show_remote_logs
    fail "remote docker compose up failed"
  fi

  wait_for_remote_health
  run_remote_compose ps
  print_remote_access_hint
}

cmd_publish_hub() {
  local image_repo
  local tag_base
  local image_ref
  local manifest_ref
  local manifest_digest
  local latest_ref
  local arch_refs=()
  local arch

  require_cmd docker

  image_repo="${FN_KNOCK_DOCKER_IMAGE_REPO:-}"
  [ -n "${image_repo}" ] || fail "FN_KNOCK_DOCKER_IMAGE_REPO is required, for example kcilnk/fn-knock"
  require_publish_image_repo "${image_repo}"

  tag_base="${FN_KNOCK_DOCKER_IMAGE_TAG:-$(build_default_publish_tag_base)}"
  tag_base="$(normalize_tag_base "${tag_base}")"
  manifest_ref="${image_repo}:${tag_base}"
  latest_ref="${image_repo}:latest"

  log "Publishing Docker Hub images for ${image_repo}"
  log "Version tag ${manifest_ref}"
  log "Additional tag ${latest_ref}"

  for arch in "${DOCKER_ARCHES[@]}"; do
    image_ref="$(build_arch_image_ref "${image_repo}" "${tag_base}" "${arch}")"
    arch_refs+=("${image_ref}")
    pushx_image "${arch}" "${image_ref}"
  done

  create_manifest_tag "${manifest_ref}" "${arch_refs[@]}"
  verify_manifest_platforms "${manifest_ref}"
  manifest_digest="$(resolve_manifest_digest "${manifest_ref}")"
  log "Version manifest digest ${manifest_digest}"
  create_manifest_tag "${latest_ref}" "${image_repo}@${manifest_digest}"
  wait_for_manifest_digest "${latest_ref}" "${manifest_digest}" "${manifest_ref}"
  verify_manifest_platforms "${latest_ref}"
}

usage() {
  cat <<'EOF'
Usage:
  bash ./scripts/fn-knock-docker.sh <command>

Commands:
  build-local   Build the local Docker image defined by deploy/docker/.env or .env.example
  up-local      Start the local Docker stack with compose.override.yaml
  down-local    Stop the local Docker stack
  logs-local    Tail local fn-knock container logs
  reset-panel-password-local   Clear Docker admin panel password for the local compose stack
  local-deploy-fast  Build and upload only the remote host architecture, then restart remote compose
  local-deploy  Build amd64, arm64, and arm32 images, upload them via SSH, and restart remote compose
  publish-hub   Push amd64, arm64, and arm32 images to a registry, then update version and latest manifest tags
  remote-ps     Show remote compose status
  remote-logs   Tail remote fn-knock container logs
  reset-panel-password-remote  Clear Docker admin panel password on the remote compose stack

Optional env overrides:
  FN_KNOCK_DOCKER_ENV_FILE        (default: deploy/docker/.env, fallback: deploy/docker/.env.example)
  FN_KNOCK_DOCKER_IMAGE           (override local build image)
  FN_KNOCK_DOCKER_IMAGE_REPO      (default: fn-knock; publish-hub requires namespace/repo)
  FN_KNOCK_DOCKER_IMAGE_TAG       (base tag; fast deploy appends remote arch, full deploy/publish append all arches)
  FN_KNOCK_DOCKER_LOCAL_ARCH      (override local build arch; default: host arch)
  FN_KNOCK_DOCKER_CACHE_DIR       (default: $HOME/.cache/fn-knock-buildx)
  FN_KNOCK_DOCKER_BUILDER         (optional docker buildx builder name)
  FN_KNOCK_DOCKER_MANAGED_BUILDER (default: fn-knock-buildx)
  FN_KNOCK_DOCKER_HTTP_PROXY      (optional build proxy; falls back to HTTP_PROXY/http_proxy)
  FN_KNOCK_DOCKER_HTTPS_PROXY     (optional build proxy; falls back to HTTPS_PROXY/https_proxy)
  FN_KNOCK_DOCKER_ALL_PROXY       (optional build proxy; falls back to ALL_PROXY/all_proxy)
  FN_KNOCK_DOCKER_NO_PROXY        (optional no_proxy; falls back to NO_PROXY/no_proxy)
  FN_KNOCK_DOCKER_PROXY_HOST_ALIAS(default: host.docker.internal; rewrites localhost/127.0.0.1 for containers)
  FN_KNOCK_DOCKER_MANIFEST_VERIFY_ATTEMPTS (default: 6)
  FN_KNOCK_DOCKER_MANIFEST_VERIFY_DELAY    (default: 5 seconds)
  FN_KNOCK_DOCKER_REMOTE_HOST     (default: root@192.168.31.135)
  FN_KNOCK_DOCKER_REMOTE_DIR      (default: /opt/fn-knock-docker)
  FN_KNOCK_DOCKER_SERVICE_NAME    (default: fn-knock)
  FN_KNOCK_DOCKER_WAIT_TIMEOUT    (default: 180)
EOF
}

cd "${ROOT_DIR}"

case "${1:-}" in
  build-local)
    cmd_build_local
    ;;
  up-local)
    cmd_up_local
    ;;
  down-local)
    cmd_down_local
    ;;
  logs-local)
    cmd_logs_local
    ;;
  reset-panel-password-local)
    cmd_reset_panel_password_local
    ;;
  local-deploy-fast)
    cmd_local_deploy_fast
    ;;
  local-deploy)
    cmd_local_deploy
    ;;
  publish-hub)
    cmd_publish_hub
    ;;
  remote-ps)
    cmd_remote_ps
    ;;
  remote-logs)
    cmd_remote_logs
    ;;
  reset-panel-password-remote)
    cmd_reset_panel_password_remote
    ;;
  *)
    usage
    exit 1
    ;;
esac
