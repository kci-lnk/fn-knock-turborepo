#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/version.sh"

GO_REPOSITORY="${1:-${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}}"
OUTPUT_DIR="${2:-${FN_KNOCK_GO_RELEASE_OUTPUT_DIR:-${ROOT_DIR}/dist/fn-knock-go-release}}"
VERSION="${FN_KNOCK_VERSION:-$(fn_knock_app_version "${ROOT_DIR}")}"
COMMIT="${FN_KNOCK_GO_SOURCE_COMMIT:-}"
READELF_BIN=""

log() {
  printf '[fn-knock-go-release] %s\n' "$*"
}

fail() {
  printf '[fn-knock-go-release] ERROR: %s\n' "$*" >&2
  exit 1
}

validate_binary() {
  local path="$1"
  local target="$2"
  local info

  [ -s "${path}" ] || fail "missing build output: ${path}"
  info="$(file -b "${path}")"
  case "${target}" in
    linux-amd64)
      printf '%s\n' "${info}" | grep -Eq 'ELF 64-bit LSB.*x86-64.*(static|Go BuildID)' || fail "${target}: ${info}"
      ;;
    linux-arm64)
      printf '%s\n' "${info}" | grep -Eq 'ELF 64-bit LSB.*(ARM aarch64|aarch64).*(static|Go BuildID)' || fail "${target}: ${info}"
      ;;
    linux-arm)
      printf '%s\n' "${info}" | grep -Eq 'ELF 32-bit LSB.*ARM.*(static|Go BuildID)' || fail "${target}: ${info}"
      ;;
    windows-amd64)
      printf '%s\n' "${info}" | grep -Eq 'PE32\+ executable.*x86-64' || fail "${target}: ${info}"
      ;;
    *)
      fail "unknown validation target: ${target}"
      ;;
  esac

  if [ "${target}" != "windows-amd64" ] && [ -n "${READELF_BIN}" ]; then
    local elf_header
    local dynamic_section
    elf_header="$("${READELF_BIN}" -h "${path}")"
    case "${target}" in
      linux-amd64)
        printf '%s\n' "${elf_header}" | grep -Eq 'Machine:[[:space:]]*(Advanced Micro Devices X86-64|AMD x86-64)' || \
          fail "${target}: readelf reported an unexpected machine"
        ;;
      linux-arm64)
        printf '%s\n' "${elf_header}" | grep -Eq 'Machine:[[:space:]]*AArch64' || \
          fail "${target}: readelf reported an unexpected machine"
        ;;
      linux-arm)
        printf '%s\n' "${elf_header}" | grep -Eq 'Machine:[[:space:]]*ARM' || \
          fail "${target}: readelf reported an unexpected machine"
        ;;
    esac
    dynamic_section="$("${READELF_BIN}" -d "${path}" 2>&1 || true)"
    if printf '%s\n' "${dynamic_section}" | grep -q '(NEEDED)'; then
      fail "${target}: CGO-disabled Go output has dynamic dependencies"
    fi
  elif [ "${target}" != "windows-amd64" ] && [ "${CI:-}" = "true" ]; then
    fail "readelf is required for CI ELF validation"
  fi
}

build_target() {
  local goos="$1"
  local goarch="$2"
  local goarm="$3"
  local output_name="$4"
  local target_label="$5"
  local output="${OUTPUT_DIR}/${output_name}"

  log "building ${target_label}"
  (
    cd "${GO_REPOSITORY}"
    export CGO_ENABLED=0
    export GOOS="${goos}"
    export GOARCH="${goarch}"
    export GOFLAGS="-mod=readonly"
    if [ -n "${goarm}" ]; then
      export GOARM="${goarm}"
    else
      unset GOARM || true
    fi
    go build \
      -trimpath \
      -ldflags "-s -w -X go-reauth-proxy/pkg/version.Version=${VERSION} -X go-reauth-proxy/pkg/version.Commit=${COMMIT}" \
      -o "${output}" \
      ./cmd/server
  )
  printf '%s\n' "${VERSION}" > "${output}.version"
  validate_binary "${output}" "${target_label}"
}

[ -d "${GO_REPOSITORY}/.git" ] || fail "Go repository is missing: ${GO_REPOSITORY}"
command -v go >/dev/null 2>&1 || fail "missing required command: go"
command -v file >/dev/null 2>&1 || fail "missing required command: file"
if command -v readelf >/dev/null 2>&1; then
  READELF_BIN="$(command -v readelf)"
elif command -v greadelf >/dev/null 2>&1; then
  READELF_BIN="$(command -v greadelf)"
fi

ACTUAL_COMMIT="$(git -C "${GO_REPOSITORY}" rev-parse HEAD)"
if [ -z "${COMMIT}" ]; then
  COMMIT="${ACTUAL_COMMIT}"
fi
printf '%s\n' "${COMMIT}" | grep -Eq '^[0-9a-fA-F]{40}$' || fail "invalid Go commit: ${COMMIT}"
ACTUAL_COMMIT_LOWER="$(printf '%s' "${ACTUAL_COMMIT}" | tr '[:upper:]' '[:lower:]')"
COMMIT_LOWER="$(printf '%s' "${COMMIT}" | tr '[:upper:]' '[:lower:]')"
[ "${ACTUAL_COMMIT_LOWER}" = "${COMMIT_LOWER}" ] || \
  fail "Go checkout ${ACTUAL_COMMIT} does not match ${COMMIT}"

mkdir -p "${OUTPUT_DIR}"
if [ "${FN_KNOCK_GO_SKIP_TESTS:-0}" != "1" ]; then
  log "running Go tests"
  (
    cd "${GO_REPOSITORY}"
    GOFLAGS="-mod=readonly" go test ./...
  )
fi

build_target linux amd64 "" go-reauth-proxy-linux-amd64 linux-amd64
build_target linux arm64 "" go-reauth-proxy-linux-arm64 linux-arm64
build_target linux arm 7 go-reauth-proxy-linux-arm linux-arm
build_target windows amd64 "" go-reauth-proxy-windows-amd64.exe windows-amd64

printf '%s\n' "${COMMIT}" > "${OUTPUT_DIR}/gateway-commit.txt"
log "release binaries are ready in ${OUTPUT_DIR}"
