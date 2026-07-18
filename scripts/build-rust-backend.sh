#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-}"
ARCH="${2:-}"
OUTPUT="${3:-}"
MANIFEST="${ROOT_DIR}/apps/server-admin-rs/Cargo.toml"
GLIBC_VERSION="${FN_KNOCK_ZIG_GLIBC_VERSION:-2.17}"
MUSL_IMAGE_AMD64="${FN_KNOCK_RUST_MUSL_IMAGE_AMD64:-messense/rust-musl-cross:x86_64-musl@sha256:ce75e9174325d4fbb3de85c309e2d7ca29f7500169bc4b5d2c611ff7e86d549a}"
MUSL_IMAGE_ARM64="${FN_KNOCK_RUST_MUSL_IMAGE_ARM64:-messense/rust-musl-cross:aarch64-musl@sha256:ecae5dd62d1c938c14f8071d36c16fa699860aace03bfb5284fb1216474d2643}"
MUSL_IMAGE_ARM="${FN_KNOCK_RUST_MUSL_IMAGE_ARM:-messense/rust-musl-cross:armv7-musleabihf@sha256:714d7529ed9098699cc13abade83c565e6a946e53975c108089a8cd3b43cb871}"
READELF_BIN=""

log() {
  printf '[fn-knock-rust-release] %s\n' "$*"
}

fail() {
  printf '[fn-knock-rust-release] ERROR: %s\n' "$*" >&2
  exit 1
}

target_for() {
  case "${MODE}:${ARCH}" in
    gnu:amd64) printf '%s\n' x86_64-unknown-linux-gnu ;;
    gnu:arm64) printf '%s\n' aarch64-unknown-linux-gnu ;;
    musl:amd64) printf '%s\n' x86_64-unknown-linux-musl ;;
    musl:arm64) printf '%s\n' aarch64-unknown-linux-musl ;;
    musl:arm) printf '%s\n' armv7-unknown-linux-musleabihf ;;
    *) fail "unsupported mode/architecture: ${MODE}:${ARCH}" ;;
  esac
}

musl_image_for() {
  case "${ARCH}" in
    amd64) printf '%s\n' "${MUSL_IMAGE_AMD64}" ;;
    arm64) printf '%s\n' "${MUSL_IMAGE_ARM64}" ;;
    arm) printf '%s\n' "${MUSL_IMAGE_ARM}" ;;
    *) fail "unsupported musl architecture: ${ARCH}" ;;
  esac
}

validate_arch() {
  local info
  local elf_header
  local dynamic_section
  info="$(file -b "${OUTPUT}")"
  case "${ARCH}" in
    amd64)
      printf '%s\n' "${info}" | grep -Eq 'ELF 64-bit LSB.*x86-64' || fail "${OUTPUT}: ${info}"
      ;;
    arm64)
      printf '%s\n' "${info}" | grep -Eq 'ELF 64-bit LSB.*(ARM aarch64|aarch64)' || fail "${OUTPUT}: ${info}"
      ;;
    arm)
      printf '%s\n' "${info}" | grep -Eq 'ELF 32-bit LSB.*ARM' || fail "${OUTPUT}: ${info}"
      ;;
  esac

  if [ "${MODE}" = "musl" ]; then
    printf '%s\n' "${info}" | grep -Eq '(statically linked|static-pie linked)' || \
      fail "musl output is not static: ${info}"
  fi

  if [ -n "${READELF_BIN}" ]; then
    elf_header="$("${READELF_BIN}" -h "${OUTPUT}")"
    case "${ARCH}" in
      amd64)
        printf '%s\n' "${elf_header}" | grep -Eq 'Machine:[[:space:]]*(Advanced Micro Devices X86-64|AMD x86-64)' || \
          fail "${OUTPUT}: readelf reported an unexpected machine"
        ;;
      arm64)
        printf '%s\n' "${elf_header}" | grep -Eq 'Machine:[[:space:]]*AArch64' || \
          fail "${OUTPUT}: readelf reported an unexpected machine"
        ;;
      arm)
        printf '%s\n' "${elf_header}" | grep -Eq 'Machine:[[:space:]]*ARM' || \
          fail "${OUTPUT}: readelf reported an unexpected machine"
        ;;
    esac
    if [ "${MODE}" = "musl" ]; then
      dynamic_section="$("${READELF_BIN}" -d "${OUTPUT}" 2>&1 || true)"
      printf '%s\n' "${dynamic_section}" | grep -q '(NEEDED)' && \
        fail "${OUTPUT}: musl output has dynamic dependencies"
    fi
  elif [ "${CI:-}" = "true" ]; then
    fail "readelf is required for CI ELF validation"
  fi
}

validate_glibc_ceiling() {
  local highest
  local permitted

  [ "${MODE}" = "gnu" ] || return 0
  [ -n "${READELF_BIN}" ] || return 0
  highest="$(
    "${READELF_BIN}" --version-info "${OUTPUT}" 2>/dev/null |
      grep -Eo 'GLIBC_[0-9]+(\.[0-9]+)+' |
      sed 's/^GLIBC_//' |
      sort -Vu |
      tail -n1
  )"
  [ -n "${highest}" ] || return 0
  permitted="$(printf '%s\n%s\n' "${highest}" "${GLIBC_VERSION}" | sort -V | head -n1)"
  [ "${permitted}" = "${highest}" ] || \
    fail "${OUTPUT} requires GLIBC_${highest}, above GLIBC_${GLIBC_VERSION}"
}

[ -n "${MODE}" ] && [ -n "${ARCH}" ] && [ -n "${OUTPUT}" ] || \
  fail "usage: $0 <gnu|musl> <amd64|arm64|arm> <output>"
command -v file >/dev/null 2>&1 || fail "missing required command: file"
if command -v readelf >/dev/null 2>&1; then
  READELF_BIN="$(command -v readelf)"
elif command -v greadelf >/dev/null 2>&1; then
  READELF_BIN="$(command -v greadelf)"
fi

TARGET="$(target_for)"
mkdir -p "$(dirname "${OUTPUT}")"

case "${MODE}" in
  gnu)
    command -v zig >/dev/null 2>&1 || fail "zig is required for GNU cross compilation"
    command -v cargo-zigbuild >/dev/null 2>&1 || fail "cargo-zigbuild is required"
    rustup target add "${TARGET}" >/dev/null
    TARGET_DIR="${ROOT_DIR}/dist/server-admin-rs-target/release-gnu-${ARCH}"
    log "building ${TARGET}.glibc-${GLIBC_VERSION}"
    CARGO_TARGET_DIR="${TARGET_DIR}" cargo zigbuild \
      --locked \
      --release \
      --manifest-path "${MANIFEST}" \
      --target "${TARGET}.${GLIBC_VERSION}"
    BUILT="$(find "${TARGET_DIR}" -type f -path '*/release/server-admin-rs' | head -n1)"
    [ -n "${BUILT}" ] || fail "cargo-zigbuild output was not found"
    cp "${BUILT}" "${OUTPUT}"
    ;;
  musl)
    command -v docker >/dev/null 2>&1 || fail "docker is required for musl compilation"
    IMAGE="$(musl_image_for)"
    TARGET_DIR="/workspace/dist/server-admin-rs-target/release-musl-${ARCH}"
    CONTAINER_OUTPUT="/workspace/${OUTPUT#${ROOT_DIR}/}"
    log "building ${TARGET} with ${IMAGE}"
    docker run --rm \
      -e CARGO_TARGET_DIR="${TARGET_DIR}" \
      -e FN_KNOCK_RUST_TARGET="${TARGET}" \
      -e FN_KNOCK_RUST_OUT="${CONTAINER_OUTPUT}" \
      -v "${ROOT_DIR}:/workspace" \
      -w /workspace \
      "${IMAGE}" \
      sh -lc 'cargo build --locked --release --manifest-path apps/server-admin-rs/Cargo.toml --target "${FN_KNOCK_RUST_TARGET}" && cp "${CARGO_TARGET_DIR}/${FN_KNOCK_RUST_TARGET}/release/server-admin-rs" "${FN_KNOCK_RUST_OUT}"'
    ;;
  *)
    fail "unsupported mode: ${MODE}"
    ;;
esac

chmod 755 "${OUTPUT}"
validate_arch
validate_glibc_ceiling
log "built ${OUTPUT}"
