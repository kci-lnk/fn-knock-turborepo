#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/version.sh"

ARTIFACTS_DIR="${FN_KNOCK_ARTIFACTS_DIR:-${ROOT_DIR}/dist/fn-knock-artifacts}"
RUNTIME_DIR="${FN_KNOCK_PREPARED_RUNTIME_DIR:-${ARTIFACTS_DIR}/runtime}"
FPK_RUST_BACKEND_DIR="${FN_KNOCK_PREPARED_FPK_RUST_BACKEND_DIR:-${ARTIFACTS_DIR}/fpk-rust-backends}"
MUSL_RUST_BACKEND_DIR="${FN_KNOCK_PREPARED_MUSL_RUST_BACKEND_DIR:-${ARTIFACTS_DIR}/musl-rust-backends}"
LINUX_DIR="${FN_KNOCK_PREPARED_LINUX_DIR:-${ARTIFACTS_DIR}/linux}"
FPK_PACKAGE_DIR="${FN_KNOCK_FPK_PACKAGE_DIR:-${ROOT_DIR}/apps/fn-knock}"
APP_DIR="${FPK_PACKAGE_DIR}/app"
DOCKER_RUST_BACKEND_DIR="${FN_KNOCK_DOCKER_RUST_BACKEND_DIR:-${ROOT_DIR}/deploy/docker/rust-backends}"
MANIFEST_FILE="${FPK_PACKAGE_DIR}/manifest"
RUST_MUSL_CROSS_IMAGE_PREFIX="${FN_KNOCK_RUST_MUSL_CROSS_IMAGE_PREFIX:-messense/rust-musl-cross}"
PREBUILT_ONLY="${FN_KNOCK_PREBUILT_ONLY:-0}"
GO_REPOSITORY="${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}"

NEED_RUNTIME=0
NEED_FPK_RUST=0
NEED_MUSL_RUST=0
NEED_LINUX=0
SYNC_APP_RUNTIME=0
SYNC_APP_FPK=0
SYNC_DOCKER=0

FPK_ARCHES=()
MUSL_ARCHES=()
RUNTIME_GATEWAY_ARCHES=()

log() {
  echo "[fn-knock-artifacts] $*"
}

fail() {
  echo "[fn-knock-artifacts] ERROR: $*" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  command -v "${cmd}" >/dev/null 2>&1 || fail "missing required command: ${cmd}"
}

normalize_fpk_arch() {
  case "$1" in
    amd64|x86|x86_64)
      printf '%s\n' "amd64"
      ;;
    arm64|aarch64)
      printf '%s\n' "arm64"
      ;;
    *)
      fail "invalid FPK architecture: $1; expected amd64/x86 or arm64"
      ;;
  esac
}

normalize_gateway_arch() {
  case "$1" in
    amd64|x86|x86_64)
      printf '%s\n' "amd64"
      ;;
    arm64|aarch64)
      printf '%s\n' "arm64"
      ;;
    arm32|armv8l|armv7|armv7l|armhf|arm)
      printf '%s\n' "arm"
      ;;
    *)
      fail "invalid gateway architecture: $1; expected amd64, arm64, or arm"
      ;;
  esac
}

read_arch_list() {
  local raw="$1"
  local normalizer="$2"
  local output_array="$3"
  local item
  local normalized
  local seen=" "

  eval "${output_array}=()"
  raw="${raw//,/ }"
  for item in ${raw}; do
    normalized="$("${normalizer}" "${item}")"
    case "${seen}" in
      *" ${normalized} "*)
        ;;
      *)
        eval "${output_array}+=(\"\${normalized}\")"
        seen="${seen}${normalized} "
        ;;
    esac
  done
}

configure_modes() {
  local raw="${*:-all}"
  local mode

  raw="${raw//,/ }"
  for mode in ${raw}; do
    case "${mode}" in
      all)
        NEED_RUNTIME=1
        NEED_FPK_RUST=1
        NEED_MUSL_RUST=1
        NEED_LINUX=1
        SYNC_APP_RUNTIME=1
        SYNC_APP_FPK=1
        SYNC_DOCKER=1
        ;;
      runtime)
        NEED_RUNTIME=1
        ;;
      fpk)
        NEED_RUNTIME=1
        NEED_FPK_RUST=1
        SYNC_APP_RUNTIME=1
        SYNC_APP_FPK=1
        ;;
      openwrt)
        NEED_RUNTIME=1
        NEED_MUSL_RUST=1
        ;;
      docker)
        NEED_RUNTIME=1
        NEED_MUSL_RUST=1
        SYNC_APP_RUNTIME=1
        SYNC_DOCKER=1
        ;;
      linux)
        NEED_RUNTIME=1
        NEED_MUSL_RUST=1
        NEED_LINUX=1
        ;;
      *)
        fail "unknown artifact mode: ${mode}; expected all, runtime, fpk, openwrt, docker, or linux"
        ;;
    esac
  done
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

  printf '%s\n' "${count}"
}

configure_rust_build_parallelism() {
  local parallel_release="${FN_KNOCK_RUST_PARALLEL_RELEASE:-0}"
  local cpu_count

  if [ -n "${CARGO_BUILD_JOBS:-}" ]; then
    log "Cargo build jobs: ${CARGO_BUILD_JOBS}"
  elif [ "${parallel_release}" = "1" ]; then
    cpu_count="$(detect_cpu_count)"
    export CARGO_BUILD_JOBS="${cpu_count}"
    log "Cargo build jobs: ${CARGO_BUILD_JOBS}"
  fi

  if [ "${parallel_release}" != "1" ]; then
    return
  fi

  cpu_count="${CARGO_BUILD_JOBS:-$(detect_cpu_count)}"
  export CARGO_PROFILE_RELEASE_LTO="${CARGO_PROFILE_RELEASE_LTO:-thin}"
  export CARGO_PROFILE_RELEASE_CODEGEN_UNITS="${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:-${cpu_count}}"

  log "Parallel release profile: lto=${CARGO_PROFILE_RELEASE_LTO}, codegen-units=${CARGO_PROFILE_RELEASE_CODEGEN_UNITS}"
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

validate_elf_arch() {
  local bin="$1"
  local arch="$2"
  local label="$3"
  local file_info

  [ -x "${bin}" ] || fail "${label} is missing or not executable: ${bin}"
  file_info="$(file -b "${bin}")"
  case "${arch}" in
    amd64)
      printf '%s\n' "${file_info}" | grep -Eq 'ELF 64-bit LSB.*x86-64' || \
        fail "${label} is not a Linux x86-64 ELF: ${file_info}"
      ;;
    arm64)
      printf '%s\n' "${file_info}" | grep -Eq 'ELF 64-bit LSB.*(ARM aarch64|aarch64)' || \
        fail "${label} is not a Linux arm64 ELF: ${file_info}"
      ;;
    arm)
      printf '%s\n' "${file_info}" | grep -Eq 'ELF 32-bit LSB.*ARM' || \
        fail "${label} is not a Linux armv7 ELF: ${file_info}"
      ;;
    *)
      fail "unsupported architecture verifier: ${arch}"
      ;;
  esac
}

rust_backend_is_fresh() {
  local bin="$1"
  local commit_file="${bin}.gateway-commit"
  local version_file="${bin}.version"
  local expected_version

  [ "${FN_KNOCK_FORCE_ARTIFACT_REBUILD:-0}" != "1" ] || return 1
  [ -f "${bin}" ] || return 1
  expected_version="$(fn_knock_app_version "${ROOT_DIR}")" || return 1
  [ -f "${version_file}" ] || return 1
  [ "$(tr -d '\r\n' < "${version_file}")" = "${expected_version}" ] || return 1
  if [ -n "${FN_KNOCK_GATEWAY_COMMIT:-}" ]; then
    [ -f "${commit_file}" ] || return 1
    [ "$(tr -d '\r\n' < "${commit_file}")" = "${FN_KNOCK_GATEWAY_COMMIT}" ] || return 1
  fi
  if find "${ROOT_DIR}/apps/server-admin-rs" \
    \( -path "${ROOT_DIR}/apps/server-admin-rs/target" -o -path "${ROOT_DIR}/apps/server-admin-rs/target/*" \) -prune \
    -o \( -name '*.rs' -o -name '*.proto' -o -name 'Cargo.toml' -o -name 'Cargo.lock' -o -name 'build.rs' -o -name 'server_i18n.json' \) \
    -newer "${bin}" -print -quit | grep -q .; then
    return 1
  fi
  return 0
}

log_binary_size() {
  local bin="$1"
  local label="$2"
  local bytes

  bytes="$(file_size_bytes "${bin}")"
  log "${label} size: $(format_bytes "${bytes}")"
}

copy_existing_rust_backend() {
  local arch="$1"
  local out_bin="$2"
  local label="$3"
  shift 3
  local candidate

  for candidate in "$@"; do
    [ -n "${candidate}" ] || continue
    [ -f "${candidate}" ] || continue
    rust_backend_is_fresh "${candidate}" || continue
    validate_elf_arch "${candidate}" "${arch}" "${label}"
    mkdir -p "$(dirname "${out_bin}")"
    if [ "${candidate}" != "${out_bin}" ]; then
      cp "${candidate}" "${out_bin}"
      cp "${candidate}.gateway-commit" "${out_bin}.gateway-commit"
      cp "${candidate}.version" "${out_bin}.version"
      chmod 755 "${out_bin}"
    fi
    validate_elf_arch "${out_bin}" "${arch}" "${label}"
    log "Reusing ${label} from ${candidate}"
    log_binary_size "${out_bin}" "${label}"
    return 0
  done

  return 1
}

sync_versions() {
  fn_knock_sync_manifest_version "${ROOT_DIR}" "${MANIFEST_FILE}" "[fn-knock-artifacts]"
  fn_knock_sync_rust_package_version "${ROOT_DIR}" "[fn-knock-artifacts]"
}

resolve_gateway_commit() {
  local worktree_state

  [ "${PREBUILT_ONLY}" != "1" ] || return 0
  [ -d "${GO_REPOSITORY}" ] || fail "missing Go-Reauth-Proxy checkout: ${GO_REPOSITORY}"
  FN_KNOCK_GATEWAY_COMMIT="$(git -C "${GO_REPOSITORY}" rev-parse HEAD 2>/dev/null)" || \
    fail "unable to resolve Go gateway commit from ${GO_REPOSITORY}"
  [[ "${FN_KNOCK_GATEWAY_COMMIT}" =~ ^[0-9a-f]{40}$ ]] || \
    fail "Go gateway commit must be a 40-character lowercase Git commit"
  worktree_state="$(git -C "${GO_REPOSITORY}" status --porcelain --untracked-files=normal)"
  [ -z "${worktree_state}" ] || \
    fail "Go gateway working tree is not clean; commit or discard changes before preparing artifacts"
  export FN_KNOCK_GATEWAY_COMMIT
  log "Locked Go gateway commit ${FN_KNOCK_GATEWAY_COMMIT}"
}

build_runtime() {
  [ "${NEED_RUNTIME}" = "1" ] || return 0

  if [ "${PREBUILT_ONLY}" = "1" ]; then
    [ -d "${RUNTIME_DIR}/ui/www" ] || fail "missing prebuilt admin UI: ${RUNTIME_DIR}/ui/www"
    [ -d "${RUNTIME_DIR}/server-auth-view/dist" ] || fail "missing prebuilt auth UI: ${RUNTIME_DIR}/server-auth-view/dist"
    [ -f "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip" ] || fail "missing prebuilt ACME bundle"
    for arch in "${RUNTIME_GATEWAY_ARCHES[@]}"; do
      validate_elf_arch \
        "${RUNTIME_DIR}/server/go-reauth-proxy-linux-${arch}" \
        "${arch}" \
        "prebuilt Go gateway ${arch}"
    done
    log "Using prebuilt runtime: ${RUNTIME_DIR}"
    return
  fi

  require_cmd rsync
  log "Preparing runtime assets -> ${RUNTIME_DIR}"
  FN_KNOCK_BUILD_RUST_BACKEND=0 \
    FN_KNOCK_FORCE_FRONTEND_REBUILD="${FN_KNOCK_FORCE_FRONTEND_REBUILD:-0}" \
    FN_KNOCK_RUNTIME_GATEWAY_ARCHES="${RUNTIME_GATEWAY_ARCHES[*]}" \
    bash "${ROOT_DIR}/scripts/assemble-runtime.sh" "${RUNTIME_DIR}"
}

resolve_fpk_rust_builder() {
  local builder="${FN_KNOCK_FPK_RUST_BUILDER:-auto}"

  if [ "${builder}" = "auto" ]; then
    if command -v zig >/dev/null 2>&1 && cargo zigbuild --help >/dev/null 2>&1; then
      builder="zig"
    else
      builder="docker"
    fi
  fi

  case "${builder}" in
    zig|docker)
      printf '%s\n' "${builder}"
      ;;
    *)
      fail "unsupported FN_KNOCK_FPK_RUST_BUILDER=${builder}; expected auto, zig, or docker"
      ;;
  esac
}

require_local_zigbuild() {
  require_cmd zig
  cargo zigbuild --help >/dev/null 2>&1 || \
    fail "cargo-zigbuild is required; install it with: cargo install cargo-zigbuild"
}

build_fpk_rust_backend_with_docker() {
  local arch="$1"
  local platform="$2"
  local out_bin="${FPK_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}"
  local image="${FN_KNOCK_RUST_DOCKER_IMAGE:-rust:1-bookworm}"
  local cargo_env_name
  local docker_env_args=(
    -e CARGO_HOME=/workspace/dist/cargo-home
    -e CARGO_TARGET_DIR="/workspace/dist/server-admin-rs-target/fpk-${arch}"
    -e FN_KNOCK_GATEWAY_COMMIT="${FN_KNOCK_GATEWAY_COMMIT}"
    -e FN_KNOCK_RUST_OUT="/workspace/${out_bin#${ROOT_DIR}/}"
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

  require_cmd docker
  mkdir -p "${FPK_RUST_BACKEND_DIR}"
  log "Building FPK Rust backend ${arch} with Docker (${platform})"
  docker run --rm \
    --platform "${platform}" \
    "${docker_env_args[@]}" \
    -v "${ROOT_DIR}:/workspace" \
    -w /workspace \
    "${image}" \
    bash -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo build --locked --release --manifest-path apps/server-admin-rs/Cargo.toml --bin server-admin-rs && cp "${CARGO_TARGET_DIR}/release/server-admin-rs" "${FN_KNOCK_RUST_OUT}" && { strip --strip-unneeded "${FN_KNOCK_RUST_OUT}" 2>/dev/null || true; }'

  chmod 755 "${out_bin}"
  printf '%s\n' "${FN_KNOCK_GATEWAY_COMMIT}" > "${out_bin}.gateway-commit"
  fn_knock_app_version "${ROOT_DIR}" > "${out_bin}.version"
  validate_elf_arch "${out_bin}" "${arch}" "FPK Rust backend ${arch}"
  log_binary_size "${out_bin}" "FPK Rust backend ${arch}"
}

build_fpk_rust_backend_with_zig() {
  local arch="$1"
  local target_triple="$2"
  local out_bin="${FPK_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}"
  local target_dir="${ROOT_DIR}/dist/server-admin-rs-target/fpk-zig-${arch}"
  local target_arg="${target_triple}"
  local glibc_version="${FN_KNOCK_ZIG_GLIBC_VERSION:-}"
  local built_bin

  if [ -n "${glibc_version}" ]; then
    target_arg="${target_triple}.${glibc_version}"
  fi

  require_local_zigbuild
  mkdir -p "${FPK_RUST_BACKEND_DIR}"
  log "Building FPK Rust backend ${arch} with cargo-zigbuild (${target_arg})"
  rustup target add "${target_triple}" >/dev/null
  CARGO_TARGET_DIR="${target_dir}" cargo zigbuild \
    --locked \
    --release \
    --manifest-path "${ROOT_DIR}/apps/server-admin-rs/Cargo.toml" \
    --bin server-admin-rs \
    --target "${target_arg}"

  built_bin="$(find "${target_dir}" -type f -path '*/release/server-admin-rs' | head -n1)"
  [ -n "${built_bin}" ] || fail "cargo-zigbuild finished but server-admin-rs was not found under ${target_dir}"
  cp "${built_bin}" "${out_bin}"
  chmod 755 "${out_bin}"
  printf '%s\n' "${FN_KNOCK_GATEWAY_COMMIT}" > "${out_bin}.gateway-commit"
  fn_knock_app_version "${ROOT_DIR}" > "${out_bin}.version"
  validate_elf_arch "${out_bin}" "${arch}" "FPK Rust backend ${arch}"
  log_binary_size "${out_bin}" "FPK Rust backend ${arch}"
}

build_fpk_rust_backends() {
  local builder
  local arch
  local out_bin

  [ "${NEED_FPK_RUST}" = "1" ] || return 0
  require_cmd file
  if [ "${PREBUILT_ONLY}" = "1" ]; then
    for arch in "${FPK_ARCHES[@]}"; do
      validate_elf_arch \
        "${FPK_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}" \
        "${arch}" \
        "prebuilt FPK Rust backend ${arch}"
    done
    log "Using prebuilt FPK Rust backends: ${FPK_RUST_BACKEND_DIR}"
    return
  fi
  configure_rust_build_parallelism
  builder="$(resolve_fpk_rust_builder)"
  log "Preparing FPK Rust backends (${FPK_ARCHES[*]}, builder: ${builder}) -> ${FPK_RUST_BACKEND_DIR}"

  for arch in "${FPK_ARCHES[@]}"; do
    out_bin="${FPK_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}"
    if rust_backend_is_fresh "${out_bin}"; then
      validate_elf_arch "${out_bin}" "${arch}" "FPK Rust backend ${arch}"
      log "Reusing FPK Rust backend ${arch}: ${out_bin}"
      log_binary_size "${out_bin}" "FPK Rust backend ${arch}"
      continue
    fi
    if copy_existing_rust_backend \
      "${arch}" \
      "${out_bin}" \
      "FPK Rust backend ${arch}" \
      "${ROOT_DIR}/dist/fn-knock-rust-backends/server-admin-rs-linux-${arch}" \
      "${APP_DIR}/server/server-admin-rs-linux-${arch}"; then
      continue
    fi

    case "${builder}:${arch}" in
      docker:amd64)
        build_fpk_rust_backend_with_docker "amd64" "linux/amd64"
        ;;
      docker:arm64)
        build_fpk_rust_backend_with_docker "arm64" "linux/arm64"
        ;;
      zig:amd64)
        build_fpk_rust_backend_with_zig "amd64" "x86_64-unknown-linux-gnu"
        ;;
      zig:arm64)
        build_fpk_rust_backend_with_zig "arm64" "aarch64-unknown-linux-gnu"
        ;;
      *)
        fail "unsupported FPK Rust backend build target: ${builder}:${arch}"
        ;;
    esac
  done
}

rust_musl_target_for_arch() {
  case "$1" in
    amd64)
      printf '%s\n' "x86_64-unknown-linux-musl"
      ;;
    arm64)
      printf '%s\n' "aarch64-unknown-linux-musl"
      ;;
    arm)
      printf '%s\n' "armv7-unknown-linux-musleabihf"
      ;;
    *)
      fail "unsupported musl architecture: $1"
      ;;
  esac
}

rust_musl_image_for_arch() {
  case "$1" in
    amd64)
      printf '%s\n' "${RUST_MUSL_CROSS_IMAGE_PREFIX}:x86_64-musl"
      ;;
    arm64)
      printf '%s\n' "${RUST_MUSL_CROSS_IMAGE_PREFIX}:aarch64-musl"
      ;;
    arm)
      printf '%s\n' "${RUST_MUSL_CROSS_IMAGE_PREFIX}:armv7-musleabihf"
      ;;
    *)
      fail "unsupported musl architecture: $1"
      ;;
  esac
}

build_musl_rust_backend() {
  local arch="$1"
  local target
  local image
  local out_bin="${MUSL_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}"

  target="$(rust_musl_target_for_arch "${arch}")"
  image="$(rust_musl_image_for_arch "${arch}")"
  mkdir -p "${MUSL_RUST_BACKEND_DIR}"
  mkdir -p "${ROOT_DIR}/dist/cargo-registry-musl" "${ROOT_DIR}/dist/cargo-git-musl"

  log "Building musl Rust backend ${arch} with ${image} (${target})"
  docker run --rm \
    -e CARGO_TARGET_DIR="/workspace/dist/server-admin-rs-target/musl-${arch}" \
    -e FN_KNOCK_GATEWAY_COMMIT="${FN_KNOCK_GATEWAY_COMMIT}" \
    -e FN_KNOCK_RUST_TARGET="${target}" \
    -e FN_KNOCK_RUST_OUT="/workspace/${out_bin#${ROOT_DIR}/}" \
    -v "${ROOT_DIR}/dist/cargo-registry-musl:/root/.cargo/registry" \
    -v "${ROOT_DIR}/dist/cargo-git-musl:/root/.cargo/git" \
    -v "${ROOT_DIR}:/workspace" \
    -w /workspace \
    "${image}" \
    sh -lc 'cargo build --locked --release --manifest-path apps/server-admin-rs/Cargo.toml --target "${FN_KNOCK_RUST_TARGET}" --bin server-admin-rs && cp "${CARGO_TARGET_DIR}/${FN_KNOCK_RUST_TARGET}/release/server-admin-rs" "${FN_KNOCK_RUST_OUT}" && { "${FN_KNOCK_RUST_TARGET}-strip" --strip-unneeded "${FN_KNOCK_RUST_OUT}" 2>/dev/null || strip --strip-unneeded "${FN_KNOCK_RUST_OUT}" 2>/dev/null || true; }'

  chmod 755 "${out_bin}"
  printf '%s\n' "${FN_KNOCK_GATEWAY_COMMIT}" > "${out_bin}.gateway-commit"
  fn_knock_app_version "${ROOT_DIR}" > "${out_bin}.version"
  validate_elf_arch "${out_bin}" "${arch}" "musl Rust backend ${arch}"
  log_binary_size "${out_bin}" "musl Rust backend ${arch}"
}

build_musl_rust_backends() {
  local arch
  local out_bin

  [ "${NEED_MUSL_RUST}" = "1" ] || return 0
  require_cmd file
  if [ "${PREBUILT_ONLY}" = "1" ]; then
    for arch in "${MUSL_ARCHES[@]}"; do
      validate_elf_arch \
        "${MUSL_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}" \
        "${arch}" \
        "prebuilt musl Rust backend ${arch}"
    done
    log "Using prebuilt musl Rust backends: ${MUSL_RUST_BACKEND_DIR}"
    return
  fi
  require_cmd docker
  log "Preparing musl Rust backends (${MUSL_ARCHES[*]}) -> ${MUSL_RUST_BACKEND_DIR}"

  for arch in "${MUSL_ARCHES[@]}"; do
    out_bin="${MUSL_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}"
    if rust_backend_is_fresh "${out_bin}"; then
      validate_elf_arch "${out_bin}" "${arch}" "musl Rust backend ${arch}"
      log "Reusing musl Rust backend ${arch}: ${out_bin}"
      log_binary_size "${out_bin}" "musl Rust backend ${arch}"
      continue
    fi
    if copy_existing_rust_backend \
      "${arch}" \
      "${out_bin}" \
      "musl Rust backend ${arch}" \
      "${DOCKER_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}" \
      "${ROOT_DIR}/dist/openwrt/rust-backends/server-admin-rs-linux-${arch}"; then
      continue
    fi
    build_musl_rust_backend "${arch}"
  done
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  fi
}

create_linux_archive() {
  local stage_dir="$1"
  local archive="$2"
  local temp_archive="${archive}.tmp"
  local -a owner_args

  find "${stage_dir}/fn-knock" -depth -exec touch -t 197001010000 {} +
  if tar --version 2>/dev/null | grep -qi bsdtar; then
    owner_args=(--uid 0 --gid 0 --uname root --gname root)
  else
    owner_args=(--owner=0 --group=0 --numeric-owner --sort=name)
  fi

  rm -f "${temp_archive}"
  COPYFILE_DISABLE=1 tar "${owner_args[@]}" -cf - -C "${stage_dir}" fn-knock | gzip -n -9 > "${temp_archive}"
  tar -tzf "${temp_archive}" > "${temp_archive}.list"
  grep -qx 'fn-knock/release.json' "${temp_archive}.list" || fail "invalid Linux archive layout: ${temp_archive}"
  rm -f "${temp_archive}.list"
  mv "${temp_archive}" "${archive}"
}

build_linux_packages() {
  local version arch stage_root release_root archive checksum

  [ "${NEED_LINUX}" = "1" ] || return 0
  require_cmd tar
  require_cmd gzip
  require_cmd rsync
  require_cmd openssl
  version="$(fn_knock_app_version "${ROOT_DIR}")"
  mkdir -p "${LINUX_DIR}"
  rm -f "${LINUX_DIR}"/fn-knock-linux-*.tar.gz "${LINUX_DIR}"/fn-knock-linux-*.tar.gz.sha256

  for arch in "${MUSL_ARCHES[@]}"; do
    stage_root="$(mktemp -d "${ARTIFACTS_DIR}/linux-stage-${arch}.XXXXXX")"
    release_root="${stage_root}/fn-knock"
    archive="${LINUX_DIR}/fn-knock-linux-${version}-${arch}.tar.gz"
    mkdir -p \
      "${release_root}/bin" \
      "${release_root}/config" \
      "${release_root}/openrc" \
      "${release_root}/systemd" \
      "${release_root}/ui/www" \
      "${release_root}/server-auth-view/dist" \
      "${release_root}/server/server-admin/resources" \
      "${release_root}/install"

    cp "${RUNTIME_DIR}/server/go-reauth-proxy-linux-${arch}" "${release_root}/bin/go-reauth-proxy"
    cp "${MUSL_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}" "${release_root}/bin/server-admin-rs"
    cp "${ROOT_DIR}/deploy/linux/fn-knock-entrypoint" "${release_root}/bin/fn-knock-entrypoint"
    cp "${ROOT_DIR}/deploy/linux/knock" "${release_root}/bin/knock"
    cp "${ROOT_DIR}/deploy/linux/fn-knock.env" "${release_root}/config/fn-knock.env"
    cp "${ROOT_DIR}/deploy/linux/fn-knock.service" "${release_root}/systemd/fn-knock.service"
    cp "${ROOT_DIR}/deploy/linux/fn-knock.openrc" "${release_root}/openrc/fn-knock"
    cp "${ROOT_DIR}/deploy/linux/install.sh" "${release_root}/install/install.sh"
    rsync -a "${RUNTIME_DIR}/ui/www/" "${release_root}/ui/www/"
    rsync -a "${RUNTIME_DIR}/server-auth-view/dist/" "${release_root}/server-auth-view/dist/"
    cp "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip" \
      "${release_root}/server/server-admin/resources/acmesh.zip"
    chmod 755 \
      "${release_root}/bin/go-reauth-proxy" \
      "${release_root}/bin/server-admin-rs" \
      "${release_root}/bin/fn-knock-entrypoint" \
      "${release_root}/bin/knock" \
      "${release_root}/openrc/fn-knock" \
      "${release_root}/install/install.sh"

    cat > "${release_root}/release.json" <<EOF
{
  "version": "${version}",
  "architecture": "${arch}",
  "runtime_target": "linux"
}
EOF

    validate_elf_arch "${release_root}/bin/go-reauth-proxy" "${arch}" "Linux gateway ${arch}"
    validate_elf_arch "${release_root}/bin/server-admin-rs" "${arch}" "Linux Rust backend ${arch}"
    create_linux_archive "${stage_root}" "${archive}"
    checksum="$(sha256_file "${archive}")"
    printf '%s  %s\n' "${checksum}" "$(basename "${archive}")" > "${archive}.sha256"
    rm -rf "${stage_root}"
    log "Linux package ${arch}: ${archive} ($(format_bytes "$(file_size_bytes "${archive}")"), sha256=${checksum})"
  done
}

sync_runtime_to_app() {
  local admin_www_dir="${APP_DIR}/ui/www"
  local auth_dist_dir="${APP_DIR}/server-auth-view/dist"
  local server_admin_dir="${APP_DIR}/server/server-admin"
  local server_dir="${APP_DIR}/server"
  local arch

  [ "${SYNC_APP_RUNTIME}" = "1" ] || return 0
  require_cmd rsync

  log "Syncing prepared runtime into ${APP_DIR}"
  mkdir -p "${admin_www_dir}" "${auth_dist_dir}" "${server_admin_dir}" "${server_dir}"
  rsync -a --delete "${RUNTIME_DIR}/ui/www/" "${admin_www_dir}/"
  rsync -a --delete "${RUNTIME_DIR}/server-auth-view/dist/" "${auth_dist_dir}/"
  rsync -a --delete "${RUNTIME_DIR}/server/server-admin/" "${server_admin_dir}/"

  rm -f "${server_dir}"/go-reauth-proxy-linux-*
  for arch in "${RUNTIME_GATEWAY_ARCHES[@]}"; do
    cp "${RUNTIME_DIR}/server/go-reauth-proxy-linux-${arch}" "${server_dir}/go-reauth-proxy-linux-${arch}"
    chmod 755 "${server_dir}/go-reauth-proxy-linux-${arch}"
  done
}

sync_fpk_rust_to_app() {
  local server_dir="${APP_DIR}/server"
  local arch

  [ "${SYNC_APP_FPK}" = "1" ] || return 0
  log "Syncing FPK Rust backends into ${APP_DIR}"
  mkdir -p "${server_dir}"
  rm -f "${server_dir}/server-admin-rs" "${server_dir}"/server-admin-rs-linux-*

  for arch in "${FPK_ARCHES[@]}"; do
    cp "${FPK_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}" "${server_dir}/server-admin-rs-linux-${arch}"
    chmod 755 "${server_dir}/server-admin-rs-linux-${arch}"
  done

  cp "${FPK_RUST_BACKEND_DIR}/server-admin-rs-linux-${FPK_ARCHES[0]}" "${server_dir}/server-admin-rs"
  chmod 755 "${server_dir}/server-admin-rs"
}

sync_docker_rust_context() {
  local arch

  [ "${SYNC_DOCKER}" = "1" ] || return 0
  log "Syncing Docker musl Rust backends into ${DOCKER_RUST_BACKEND_DIR}"
  mkdir -p "${DOCKER_RUST_BACKEND_DIR}"
  rm -f "${DOCKER_RUST_BACKEND_DIR}"/server-admin-rs-linux-*

  for arch in "${MUSL_ARCHES[@]}"; do
    cp "${MUSL_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}" "${DOCKER_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}"
    chmod 755 "${DOCKER_RUST_BACKEND_DIR}/server-admin-rs-linux-${arch}"
  done
}

print_summary() {
  log "Artifacts ready"
  log "runtime: ${RUNTIME_DIR}"
  if [ "${NEED_FPK_RUST}" = "1" ]; then
    log "FPK Rust backends: ${FPK_RUST_BACKEND_DIR}"
  fi
  if [ "${NEED_MUSL_RUST}" = "1" ]; then
    log "musl Rust backends: ${MUSL_RUST_BACKEND_DIR}"
  fi
  if [ "${NEED_LINUX}" = "1" ]; then
    log "Linux packages: ${LINUX_DIR}"
  fi
}

usage() {
  cat <<'EOF'
Usage:
  bash ./scripts/fn-knock-prepare-artifacts.sh [all|runtime|fpk|openwrt|docker|linux ...]

Modes:
  all      Build runtime, FPK Rust, musl Rust, and sync FPK/Docker contexts
  runtime  Build shared frontend/ACME/Go runtime only
  fpk      Build runtime + GNU Linux Rust backends and sync apps/fn-knock/app
  openwrt  Build runtime + musl Rust backends for OpenWrt packaging
  docker   Build runtime + musl Rust backends and sync Docker build context
  linux    Build runtime + static musl Rust backends and package systemd releases

Useful env:
  FN_KNOCK_ARTIFACTS_DIR
  FN_KNOCK_FPK_ARCHES
  FN_KNOCK_MUSL_ARCHES
  FN_KNOCK_RUNTIME_GATEWAY_ARCHES
  FN_KNOCK_FORCE_ARTIFACT_REBUILD=1
  FN_KNOCK_FORCE_FRONTEND_REBUILD=1
EOF
}

case "${1:-}" in
  -h|--help|help)
    usage
    exit 0
    ;;
esac

cd "${ROOT_DIR}"
configure_modes "$@"
read_arch_list "${FN_KNOCK_FPK_ARCHES:-amd64 arm64}" normalize_fpk_arch FPK_ARCHES
read_arch_list "${FN_KNOCK_MUSL_ARCHES:-amd64 arm64 arm}" normalize_gateway_arch MUSL_ARCHES
read_arch_list "${FN_KNOCK_RUNTIME_GATEWAY_ARCHES:-amd64 arm64 arm}" normalize_gateway_arch RUNTIME_GATEWAY_ARCHES

[ "${#FPK_ARCHES[@]}" -gt 0 ] || fail "FPK architecture list is empty"
[ "${#MUSL_ARCHES[@]}" -gt 0 ] || fail "musl architecture list is empty"
[ "${#RUNTIME_GATEWAY_ARCHES[@]}" -gt 0 ] || fail "runtime gateway architecture list is empty"

sync_versions
resolve_gateway_commit
build_runtime
build_fpk_rust_backends
build_musl_rust_backends
build_linux_packages
sync_runtime_to_app
sync_fpk_rust_to_app
sync_docker_rust_context
print_summary
