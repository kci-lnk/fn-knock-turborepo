#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
source "${ROOT_DIR}/scripts/version.sh"
REMOTE_HOST="${FN_KNOCK_REMOTE_HOST:-root@192.168.31.98}"
REMOTE_DIR="${FN_KNOCK_REMOTE_DIR:-/tmp/fn-knock-fpk}"
LOCAL_FPK_PATH="${FN_KNOCK_LOCAL_FPK_PATH:-apps/fn-knock/dist/fn-knock.fpk}"
APP_NAME="${FN_KNOCK_APP_NAME:-fn-knock}"
REMOTE_FPK_AMD64_PATH="${REMOTE_DIR}/${APP_NAME}-amd64.fpk"
REMOTE_FPK_ARM64_PATH="${REMOTE_DIR}/${APP_NAME}-arm64.fpk"
VERSION_FILE="${ROOT_DIR}/version.json"
MANIFEST_FILE="${ROOT_DIR}/apps/fn-knock/manifest"

derive_arch_fpk_path() {
  local base_path="$1"
  local arch="$2"
  local dir_name
  local file_name
  local file_stem

  dir_name="$(dirname "${base_path}")"
  file_name="$(basename "${base_path}")"
  file_stem="${file_name%.fpk}"

  if [ "${file_stem}" = "${file_name}" ]; then
    echo "${dir_name}/${file_name}-${arch}.fpk"
    return 0
  fi

  echo "${dir_name}/${file_stem}-${arch}.fpk"
}

LOCAL_FPK_AMD64_PATH="$(derive_arch_fpk_path "${LOCAL_FPK_PATH}" "amd64")"
LOCAL_FPK_ARM64_PATH="$(derive_arch_fpk_path "${LOCAL_FPK_PATH}" "arm64")"
RUST_BACKEND_OUTPUT_DIR="${ROOT_DIR}/dist/fn-knock-rust-backends"
FPK_ARCHES=()

read_fpk_arches() {
  local raw="${FN_KNOCK_FPK_ARCHES:-amd64 arm64}"
  raw="${raw//,/ }"

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
      *)
        echo "[fn-knock] Invalid FPK architecture: ${arch}; expected amd64/x86 or arm64" >&2
        exit 1
        ;;
    esac

    case "${seen}" in
      *" ${normalized} "*) ;;
      *)
        FPK_ARCHES+=("${normalized}")
        seen="${seen}${normalized} "
        ;;
    esac
  done

  if [ "${#FPK_ARCHES[@]}" -eq 0 ]; then
    echo "[fn-knock] FPK architecture list is empty" >&2
    exit 1
  fi
}

fpk_arch_enabled() {
  local target="$1"
  local arch

  for arch in "${FPK_ARCHES[@]}"; do
    if [ "${arch}" = "${target}" ]; then
      return 0
    fi
  done

  return 1
}

detect_cpu_count() {
  local count

  count="$(getconf _NPROCESSORS_ONLN 2>/dev/null || true)"
  if [ -z "${count}" ]; then
    count="$(sysctl -n hw.logicalcpu 2>/dev/null || true)"
  fi
  if ! printf '%s\n' "${count}" | grep -Eq '^[1-9][0-9]*$'; then
    count="1"
  fi

  echo "${count}"
}

configure_rust_build_parallelism() {
  local parallel_release="${FN_KNOCK_RUST_PARALLEL_RELEASE:-0}"
  local cpu_count

  if [ -n "${CARGO_BUILD_JOBS:-}" ]; then
    echo "[fn-knock] Cargo build jobs: ${CARGO_BUILD_JOBS}"
  elif [ "${parallel_release}" = "1" ]; then
    cpu_count="$(detect_cpu_count)"
    export CARGO_BUILD_JOBS="${cpu_count}"
    echo "[fn-knock] Cargo build jobs: ${CARGO_BUILD_JOBS}"
  fi

  if [ "${parallel_release}" != "1" ]; then
    return
  fi

  cpu_count="${CARGO_BUILD_JOBS:-$(detect_cpu_count)}"
  export CARGO_PROFILE_RELEASE_LTO="${CARGO_PROFILE_RELEASE_LTO:-thin}"
  export CARGO_PROFILE_RELEASE_CODEGEN_UNITS="${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:-${cpu_count}}"

  echo "[fn-knock] Parallel release profile: lto=${CARGO_PROFILE_RELEASE_LTO}, codegen-units=${CARGO_PROFILE_RELEASE_CODEGEN_UNITS}"
}

sync_manifest_version() {
  fn_knock_sync_manifest_version "${ROOT_DIR}" "${MANIFEST_FILE}" "[fn-knock]"
  fn_knock_sync_rust_package_version "${ROOT_DIR}" "[fn-knock]"
}

build_package_assets() {
  cd "${ROOT_DIR}"

  echo "[fn-knock] Target FPK architectures: ${FPK_ARCHES[*]}"
  if [ "${FN_KNOCK_ARTIFACTS_ALREADY_PREPARED:-0}" = "1" ]; then
    echo "[fn-knock] Using already prepared shared artifacts for FPK package assets"
  else
    echo "[fn-knock] Preparing shared artifacts for FPK package assets..."
    FN_KNOCK_FPK_ARCHES="${FPK_ARCHES[*]}" \
      FN_KNOCK_RUNTIME_GATEWAY_ARCHES="${FPK_ARCHES[*]}" \
      bash "${ROOT_DIR}/scripts/fn-knock-prepare-artifacts.sh" fpk
  fi
  chmod +x \
    "${ROOT_DIR}/apps/fn-knock/cmd/main" \
    "${ROOT_DIR}/apps/fn-knock/app/ui/index.cgi"
  echo "[fn-knock] Package assets are ready under apps/fn-knock/app"
}

build_fpk_rust_backends() {
  if [ "${FN_KNOCK_FPK_BUILD_RUST_BACKENDS:-1}" != "1" ]; then
    echo "[fn-knock] Skipping Linux Rust backend build (FN_KNOCK_FPK_BUILD_RUST_BACKENDS=0)"
    return
  fi

  mkdir -p "${RUST_BACKEND_OUTPUT_DIR}"
  configure_rust_build_parallelism

  local builder="${FN_KNOCK_FPK_RUST_BUILDER:-auto}"
  if [ "${builder}" = "auto" ]; then
    if command -v zig >/dev/null 2>&1 && cargo zigbuild --help >/dev/null 2>&1; then
      builder="zig"
    else
      builder="docker"
    fi
  fi

  case "${builder}" in
    zig)
      require_local_zigbuild
      for arch in "${FPK_ARCHES[@]}"; do
        case "${arch}" in
          amd64)
            build_fpk_rust_backend_with_zig "amd64" "x86_64-unknown-linux-gnu"
            ;;
          arm64)
            build_fpk_rust_backend_with_zig "arm64" "aarch64-unknown-linux-gnu"
            ;;
        esac
      done
      ;;
    docker)
      if ! command -v docker >/dev/null 2>&1; then
        echo "[fn-knock] Docker is required to build Linux Rust backend binaries for FPK packaging when Zig is unavailable" >&2
        exit 1
      fi
      for arch in "${FPK_ARCHES[@]}"; do
        case "${arch}" in
          amd64)
            build_fpk_rust_backend_with_docker "amd64" "linux/amd64"
            ;;
          arm64)
            build_fpk_rust_backend_with_docker "arm64" "linux/arm64"
            ;;
        esac
      done
      ;;
    *)
      echo "[fn-knock] Unsupported FN_KNOCK_FPK_RUST_BUILDER=${builder}; expected auto, zig, or docker" >&2
      exit 1
      ;;
  esac
}

require_local_zigbuild() {
  if ! command -v zig >/dev/null 2>&1; then
    echo "[fn-knock] Zig is required for local Linux cross compilation; install zig before running the FPK build" >&2
    exit 1
  fi

  if ! cargo zigbuild --help >/dev/null 2>&1; then
    echo "[fn-knock] cargo-zigbuild is required for local Linux cross compilation; install it with: cargo install cargo-zigbuild" >&2
    exit 1
  fi
}

build_fpk_rust_backend_with_docker() {
  local arch="$1"
  local platform="$2"
  local out_bin="${RUST_BACKEND_OUTPUT_DIR}/server-admin-rs-linux-${arch}"
  local image="${FN_KNOCK_RUST_DOCKER_IMAGE:-rust:1-bookworm}"
  local cargo_env_name
  local docker_env_args=(
    -e CARGO_HOME=/workspace/dist/cargo-home
    -e CARGO_TARGET_DIR="/workspace/dist/server-admin-rs-target/${arch}"
    -e FN_KNOCK_RUST_OUT="/workspace/dist/fn-knock-rust-backends/server-admin-rs-linux-${arch}"
  )

  for cargo_env_name in \
    CARGO_BUILD_JOBS \
    CARGO_PROFILE_RELEASE_LTO \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS \
    CARGO_PROFILE_RELEASE_OPT_LEVEL \
    CARGO_PROFILE_RELEASE_INCREMENTAL \
    RUSTFLAGS
  do
    if [ -n "${!cargo_env_name:-}" ]; then
      docker_env_args+=(-e "${cargo_env_name}=${!cargo_env_name}")
    fi
  done

  echo "[fn-knock] Building server-admin-rs for ${platform} with Docker..."
  docker run --rm \
    --platform "${platform}" \
    "${docker_env_args[@]}" \
    -v "${ROOT_DIR}:/workspace" \
    -w /workspace \
    "${image}" \
    bash -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo build --release --manifest-path apps/server-admin-rs/Cargo.toml && cp "${CARGO_TARGET_DIR}/release/server-admin-rs" "${FN_KNOCK_RUST_OUT}" && { strip --strip-unneeded "${FN_KNOCK_RUST_OUT}" 2>/dev/null || true; }'

  chmod 755 "${out_bin}"
  log_rust_backend_binary_size "${out_bin}" "${arch}"
  verify_linux_rust_backend "${out_bin}" "${arch}"
  echo "[fn-knock] Prepared Rust backend ${arch}: ${out_bin}"
}

build_fpk_rust_backend_with_zig() {
  local arch="$1"
  local target_triple="$2"
  local out_bin="${RUST_BACKEND_OUTPUT_DIR}/server-admin-rs-linux-${arch}"
  local target_dir="${ROOT_DIR}/dist/server-admin-rs-target/zig-${arch}"
  local target_arg="${target_triple}"
  local glibc_version="${FN_KNOCK_ZIG_GLIBC_VERSION:-}"
  local built_bin

  if [ -n "${glibc_version}" ]; then
    target_arg="${target_triple}.${glibc_version}"
  fi

  echo "[fn-knock] Building server-admin-rs for ${target_arg} with cargo-zigbuild..."
  rustup target add "${target_triple}" >/dev/null
  CARGO_TARGET_DIR="${target_dir}" cargo zigbuild \
    --release \
    --manifest-path "${ROOT_DIR}/apps/server-admin-rs/Cargo.toml" \
    --target "${target_arg}"

  built_bin="$(find "${target_dir}" -type f -path '*/release/server-admin-rs' | head -n1)"
  if [ -z "${built_bin}" ]; then
    echo "[fn-knock] cargo-zigbuild finished but server-admin-rs was not found under ${target_dir}" >&2
    exit 1
  fi
  cp "${built_bin}" "${out_bin}"
  chmod 755 "${out_bin}"
  log_rust_backend_binary_size "${out_bin}" "${arch}"
  verify_linux_rust_backend "${out_bin}" "${arch}"
  echo "[fn-knock] Prepared Rust backend ${arch}: ${out_bin}"
}

log_rust_backend_binary_size() {
  local bin="$1"
  local arch="$2"
  local bytes

  bytes="$(file_size_bytes "${bin}")"
  echo "[fn-knock] Rust backend ${arch} size: $(format_bytes "${bytes}")"
}

file_size_bytes() {
  wc -c < "$1" | tr -d '[:space:]'
}

format_bytes() {
  local bytes="$1"
  awk -v bytes="${bytes}" 'BEGIN {
    split("B KiB MiB GiB", units, " ");
    value = bytes + 0;
    unit = 1;
    while (value >= 1024 && unit < 4) {
      value /= 1024;
      unit++;
    }
    if (unit == 1) {
      printf "%d %s", value, units[unit];
    } else {
      printf "%.1f %s", value, units[unit];
    }
  }'
}

verify_linux_rust_backend() {
  local bin="$1"
  local arch="$2"
  local file_info

  if [ ! -x "${bin}" ]; then
    echo "[fn-knock] Missing executable Rust backend: ${bin}" >&2
    exit 1
  fi

  file_info="$(file -b "${bin}")"
  case "${arch}" in
    amd64)
      if ! printf '%s\n' "${file_info}" | grep -Eq 'ELF 64-bit LSB.*x86-64'; then
        echo "[fn-knock] Rust backend ${bin} is not a Linux x86-64 ELF: ${file_info}" >&2
        exit 1
      fi
      ;;
    arm64)
      if ! printf '%s\n' "${file_info}" | grep -Eq 'ELF 64-bit LSB.*(ARM aarch64|aarch64)'; then
        echo "[fn-knock] Rust backend ${bin} is not a Linux arm64 ELF: ${file_info}" >&2
        exit 1
      fi
      ;;
    *)
      echo "[fn-knock] Unsupported Rust backend arch: ${arch}" >&2
      exit 1
      ;;
  esac
}

copy_remote_fpk() {
  cd "${ROOT_DIR}"
  mkdir -p "$(dirname "${LOCAL_FPK_AMD64_PATH}")"
  if fpk_arch_enabled "amd64"; then
    echo "[fn-knock] Pulling remote FPK: ${REMOTE_HOST}:${REMOTE_FPK_AMD64_PATH} -> ${LOCAL_FPK_AMD64_PATH}"
    scp "${REMOTE_HOST}:${REMOTE_FPK_AMD64_PATH}" "${LOCAL_FPK_AMD64_PATH}"
  fi
  if fpk_arch_enabled "arm64"; then
    echo "[fn-knock] Pulling remote FPK: ${REMOTE_HOST}:${REMOTE_FPK_ARM64_PATH} -> ${LOCAL_FPK_ARM64_PATH}"
    scp "${REMOTE_HOST}:${REMOTE_FPK_ARM64_PATH}" "${LOCAL_FPK_ARM64_PATH}"
  fi
  echo "[fn-knock] FPK copied for architectures: ${FPK_ARCHES[*]}"
}

usage() {
  cat <<'EOF'
Usage:
  ./apps/fn-knock/scripts/build-package.sh [build-assets|copy-fpk]

Commands:
  build-assets  Build and sync package assets (default)
  copy-fpk      Copy packaged FPKs from remote host to local dist paths

Optional env overrides:
  FN_KNOCK_FPK_ARCHES  Space/comma list: amd64/x86 and/or arm64 (default: amd64 arm64)
  FN_KNOCK_FPK_RUST_BUILDER  Rust backend builder: auto, zig, or docker (default: auto)
  FN_KNOCK_RUST_PARALLEL_RELEASE  Set 1 to override release LTO/codegen for more parallel builds
  CARGO_BUILD_JOBS  Cargo job count; defaults to CPU count when FN_KNOCK_RUST_PARALLEL_RELEASE=1
  CARGO_PROFILE_RELEASE_LTO  Optional Cargo release LTO override, e.g. thin
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS  Optional release codegen units override
EOF
}

read_fpk_arches

cmd="${1:-build-assets}"
case "${cmd}" in
  build-assets)
    build_package_assets
    ;;
  copy-fpk)
    copy_remote_fpk
    ;;
  *)
    usage
    exit 1
    ;;
esac
