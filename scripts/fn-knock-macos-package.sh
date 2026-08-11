#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/version.sh"

ARCH="${1:-${FN_KNOCK_MACOS_ARCH:-}}"
RUNTIME_DIR="${2:-${FN_KNOCK_PREPARED_RUNTIME_DIR:-${ROOT_DIR}/dist/fn-knock-artifacts/runtime}}"
GO_REPOSITORY="${3:-${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}}"
OUTPUT_DIR="${4:-${FN_KNOCK_MACOS_OUTPUT_DIR:-${ROOT_DIR}/dist/fn-knock-macos}}"
VERSION="${FN_KNOCK_VERSION:-$(fn_knock_app_version "${ROOT_DIR}")}"
REPOSITORY_VERSION="$(fn_knock_app_version "${ROOT_DIR}")"
SOURCE_COMMIT="${FN_KNOCK_SOURCE_COMMIT:-$(git -C "${ROOT_DIR}" rev-parse HEAD)}"
GO_COMMIT=""
MIN_MACOS_VERSION="${FN_KNOCK_MIN_MACOS_VERSION:-13.0}"
PREBUILT_ONLY="${FN_KNOCK_MACOS_PREBUILT_ONLY:-0}"
BUILD_DIR="${FN_KNOCK_MACOS_BUILD_DIR:-${ROOT_DIR}/dist/macos-build-${ARCH}}"

log() { printf '[fn-knock-macos] %s\n' "$*"; }
fail() { printf '[fn-knock-macos] ERROR: %s\n' "$*" >&2; exit 1; }

case "${ARCH}" in
  amd64) RUST_TARGET=x86_64-apple-darwin; NATIVE_ARCH=x86_64 ;;
  arm64) RUST_TARGET=aarch64-apple-darwin; NATIVE_ARCH=arm64 ;;
  *) fail "usage: $0 <amd64|arm64> [runtime-dir] [Go-Reauth-Proxy-dir] [output-dir]" ;;
esac

for command_name in file git otool lipo tar gzip shasum ditto strings plutil; do
  command -v "${command_name}" >/dev/null 2>&1 || fail "missing required command: ${command_name}"
done
[ "$(uname -s)" = Darwin ] || fail "macOS packages must be built on a native macOS runner"
[ "$(uname -m)" = "${NATIVE_ARCH}" ] || fail "${ARCH} package requires native ${NATIVE_ARCH} runner"
[ -d "${RUNTIME_DIR}/ui/www" ] || fail "missing prepared admin UI: ${RUNTIME_DIR}/ui/www"
[ -d "${RUNTIME_DIR}/server-auth-view/dist" ] || fail "missing prepared auth UI"
[ -f "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip" ] || fail "missing ACME bundle"
[ -d "${GO_REPOSITORY}/.git" ] || fail "Go gateway repository is missing: ${GO_REPOSITORY}"
printf '%s\n' "${SOURCE_COMMIT}" | grep -Eq '^[0-9a-fA-F]{40}$' || fail "invalid source commit"
printf '%s\n' "${VERSION}" | grep -Eq '^[0-9][0-9A-Za-z._+-]*$' || fail "invalid release version"
printf '%s\n' "${MIN_MACOS_VERSION}" | grep -Eq '^[0-9]+(\.[0-9]+){1,2}$' || fail "invalid minimum macOS version"
[ "${VERSION}" = "${REPOSITORY_VERSION}" ] || \
  fail "package version ${VERSION} does not match version.json ${REPOSITORY_VERSION}"
ACTUAL_SOURCE_COMMIT="$(git -C "${ROOT_DIR}" rev-parse HEAD)"
[ "$(printf '%s' "${SOURCE_COMMIT}" | tr '[:upper:]' '[:lower:]')" = "$(printf '%s' "${ACTUAL_SOURCE_COMMIT}" | tr '[:upper:]' '[:lower:]')" ] || \
  fail "source checkout ${ACTUAL_SOURCE_COMMIT} does not match ${SOURCE_COMMIT}"

ACTUAL_GO_COMMIT="$(git -C "${GO_REPOSITORY}" rev-parse HEAD)"
GO_COMMIT="${ACTUAL_GO_COMMIT}"
printf '%s\n' "${GO_COMMIT}" | grep -Eq '^[0-9a-fA-F]{40}$' || fail "invalid Go source commit"
bash "${ROOT_DIR}/scripts/verify-go-control-api-contract.sh" "${GO_REPOSITORY}"
CONTROL_API_VERSION="$(bash "${ROOT_DIR}/scripts/control-api-version.sh")"
printf '%s\n' "${CONTROL_API_VERSION}" | grep -Eq '^[1-9][0-9]*$' || fail "invalid control API version"

mkdir -p "${BUILD_DIR}" "${OUTPUT_DIR}"
GO_BIN="${BUILD_DIR}/go-reauth-proxy"
RUST_BIN="${BUILD_DIR}/server-admin-rs"

if [ "${PREBUILT_ONLY}" != "1" ]; then
  command -v go >/dev/null 2>&1 || fail "missing required command: go"
  command -v cargo >/dev/null 2>&1 || fail "missing required command: cargo"
  command -v rustup >/dev/null 2>&1 || fail "missing required command: rustup"
  if [ "${FN_KNOCK_GO_SKIP_TESTS:-0}" != "1" ]; then
    log "running Go tests"
    (cd "${GO_REPOSITORY}" && GOFLAGS=-mod=readonly go test ./...)
  fi
  log "building Go gateway for darwin/${ARCH}"
  (
    cd "${GO_REPOSITORY}"
    CGO_ENABLED=0 GOOS=darwin GOARCH="${ARCH}" GOFLAGS=-mod=readonly \
      go build -trimpath \
      -ldflags "-s -w -X go-reauth-proxy/pkg/version.Version=${VERSION} -X go-reauth-proxy/pkg/version.Commit=${GO_COMMIT}" \
      -o "${GO_BIN}" ./cmd/server
  )

  fn_knock_sync_rust_package_version "${ROOT_DIR}" "[fn-knock-macos]"
  rustup target add "${RUST_TARGET}" >/dev/null
  log "building Rust administration service for ${RUST_TARGET}"
  MACOSX_DEPLOYMENT_TARGET="${MIN_MACOS_VERSION}" \
    FN_KNOCK_GIT_COMMIT="${SOURCE_COMMIT}" \
    FN_KNOCK_GATEWAY_COMMIT="${GO_COMMIT}" \
    CARGO_TARGET_DIR="${BUILD_DIR}/cargo-target" \
    cargo build --locked --release \
      --manifest-path "${ROOT_DIR}/apps/server-admin-rs/Cargo.toml" \
      --target "${RUST_TARGET}"
  cp "${BUILD_DIR}/cargo-target/${RUST_TARGET}/release/server-admin-rs" "${RUST_BIN}"
fi

validate_macho() {
  local binary="$1" label="$2" info arches minos
  [ -x "${binary}" ] || chmod 0755 "${binary}" 2>/dev/null || true
  [ -x "${binary}" ] || fail "missing executable ${label}: ${binary}"
  info="$(file -b "${binary}")"
  case "${ARCH}" in
    amd64) printf '%s\n' "${info}" | grep -Eq 'Mach-O 64-bit executable x86_64' || fail "${label} is not x86_64 Mach-O: ${info}" ;;
    arm64) printf '%s\n' "${info}" | grep -Eq 'Mach-O 64-bit executable arm64' || fail "${label} is not arm64 Mach-O: ${info}" ;;
  esac
  arches="$(lipo -archs "${binary}")"
  [ "${arches}" = "${NATIVE_ARCH}" ] || fail "${label} must contain only ${NATIVE_ARCH}; found ${arches}"
  if otool -L "${binary}" | tail -n +2 | grep -Eq '(/opt/homebrew|/usr/local|/Users/|/private/tmp)'; then
    fail "${label} contains a build-machine or Homebrew dependency"
  fi
  minos="$(otool -l "${binary}" | awk '
    $1 == "cmd" && ($2 == "LC_BUILD_VERSION" || $2 == "LC_VERSION_MIN_MACOSX") { in_version=1; next }
    in_version && ($1 == "minos" || $1 == "version") { print $2; exit }
  ')"
  [ -n "${minos}" ] || fail "${label} does not declare a minimum macOS version"
  awk -v found="${minos}" -v ceiling="${MIN_MACOS_VERSION}" 'BEGIN {
    split(found, a, "."); split(ceiling, b, ".");
    for (i = 1; i <= 3; i++) {
      av = (a[i] == "" ? 0 : a[i] + 0); bv = (b[i] == "" ? 0 : b[i] + 0);
      if (av < bv) exit 0;
      if (av > bv) exit 1;
    }
    exit 0;
  }' || fail "${label} requires macOS ${minos}, above ${MIN_MACOS_VERSION}"
}

validate_macho "${GO_BIN}" "Go gateway"
validate_macho "${RUST_BIN}" "Rust administration service"

binary_contains() {
  local binary="$1" expected="$2" label="$3"
  strings -a "${binary}" | awk -v wanted="${expected}" '
    index($0, wanted) { found = 1 }
    END { exit found ? 0 : 1 }
  ' || fail "${label} is not embedded in $(basename "${binary}")"
}

binary_contains "${GO_BIN}" "${VERSION}" "release version"
binary_contains "${RUST_BIN}" "${VERSION}" "release version"
binary_contains "${GO_BIN}" "${GO_COMMIT}" "Go source commit"
binary_contains "${RUST_BIN}" "${SOURCE_COMMIT}" "fn-knock source commit"

STAGE_DIR="$(mktemp -d "${OUTPUT_DIR}/macos-stage-${ARCH}.XXXXXX")"
trap 'rm -rf "${STAGE_DIR}"' EXIT
RELEASE_ROOT="${STAGE_DIR}/fn-knock"
mkdir -p \
  "${RELEASE_ROOT}/bin" \
  "${RELEASE_ROOT}/config" \
  "${RELEASE_ROOT}/launchd" \
  "${RELEASE_ROOT}/install" \
  "${RELEASE_ROOT}/ui/www" \
  "${RELEASE_ROOT}/server-auth-view/dist" \
  "${RELEASE_ROOT}/server/server-admin/resources"

cp "${GO_BIN}" "${RELEASE_ROOT}/bin/go-reauth-proxy"
cp "${RUST_BIN}" "${RELEASE_ROOT}/bin/server-admin-rs"
cp "${ROOT_DIR}/deploy/macos/fn-knock-entrypoint" "${RELEASE_ROOT}/bin/fn-knock-entrypoint"
cp "${ROOT_DIR}/deploy/macos/knock" "${RELEASE_ROOT}/bin/knock"
cp "${ROOT_DIR}/deploy/macos/fn-knock.env" "${RELEASE_ROOT}/config/fn-knock.env"
cp "${ROOT_DIR}/deploy/macos/cn.fnknock.service.plist" "${RELEASE_ROOT}/launchd/cn.fnknock.service.plist"
cp "${ROOT_DIR}/deploy/macos/install.sh" "${RELEASE_ROOT}/install/install.sh"
ditto "${RUNTIME_DIR}/ui/www" "${RELEASE_ROOT}/ui/www"
ditto "${RUNTIME_DIR}/server-auth-view/dist" "${RELEASE_ROOT}/server-auth-view/dist"
cp "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip" \
  "${RELEASE_ROOT}/server/server-admin/resources/acmesh.zip"
chmod 0755 "${RELEASE_ROOT}/bin/"* "${RELEASE_ROOT}/install/install.sh"

cat > "${RELEASE_ROOT}/release.json" <<EOF
{
  "version": "${VERSION}",
  "architecture": "${ARCH}",
  "runtime_target": "macos",
  "minimum_macos_version": "${MIN_MACOS_VERSION}",
  "source_commit": "${SOURCE_COMMIT}",
  "gateway_commit": "${GO_COMMIT}",
  "control_api_version": ${CONTROL_API_VERSION},
  "apple_signed": false
}
EOF

/usr/bin/plutil -convert json -o /dev/null "${RELEASE_ROOT}/release.json" >/dev/null || fail "invalid release.json"
[ "$(/usr/bin/plutil -extract version raw -o - "${RELEASE_ROOT}/release.json")" = "${VERSION}" ] || \
  fail "release.json version mismatch"
[ "$(/usr/bin/plutil -extract architecture raw -o - "${RELEASE_ROOT}/release.json")" = "${ARCH}" ] || \
  fail "release.json architecture mismatch"
[ "$(/usr/bin/plutil -extract runtime_target raw -o - "${RELEASE_ROOT}/release.json")" = macos ] || \
  fail "release.json runtime target mismatch"
if find "${RELEASE_ROOT}" -type l -print -quit | grep -q .; then
  fail "release staging tree contains symbolic links"
fi
if find "${RELEASE_ROOT}" ! -type f ! -type d -print -quit | grep -q .; then
  fail "release staging tree contains special files"
fi

find "${RELEASE_ROOT}" -depth -exec touch -t 197001010000 {} +
ARCHIVE="${OUTPUT_DIR}/fn-knock-macos-${VERSION}-${ARCH}.tar.gz"
TEMP_ARCHIVE="${ARCHIVE}.tmp"
rm -f "${TEMP_ARCHIVE}"
COPYFILE_DISABLE=1 tar --uid 0 --gid 0 --uname root --gname root -cf - -C "${STAGE_DIR}" fn-knock | \
  gzip -n -9 > "${TEMP_ARCHIVE}"
tar -tzf "${TEMP_ARCHIVE}" > "${TEMP_ARCHIVE}.list"
grep -qx 'fn-knock/release.json' "${TEMP_ARCHIVE}.list" || fail "invalid archive layout"
if awk '
  $0 != "fn-knock" && index($0, "fn-knock/") != 1 { bad=1 }
  $0 ~ /(^|\/)\.\.?($|\/)/ || $0 ~ /[[:cntrl:]\\]/ { bad=1 }
  END { exit bad ? 0 : 1 }
' "${TEMP_ARCHIVE}.list"; then
  fail "archive contains unsafe paths"
fi
tar -tvzf "${TEMP_ARCHIVE}" > "${TEMP_ARCHIVE}.verbose"
if awk 'substr($0, 1, 1) != "-" && substr($0, 1, 1) != "d" { bad=1 } END { exit bad ? 0 : 1 }' \
  "${TEMP_ARCHIVE}.verbose"; then
  fail "archive contains links or special files"
fi
rm -f "${TEMP_ARCHIVE}.list" "${TEMP_ARCHIVE}.verbose"
mv "${TEMP_ARCHIVE}" "${ARCHIVE}"
SHA256="$(shasum -a 256 "${ARCHIVE}" | awk '{print $1}')"
rm -rf "${STAGE_DIR}"
trap - EXIT
log "package ready: ${ARCHIVE} (sha256=${SHA256})"
