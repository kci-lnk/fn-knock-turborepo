#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"
source "${ROOT_DIR}/scripts/version.sh"

REMOTE_HOST="${FN_KNOCK_REMOTE_HOST:-root@192.168.31.98}"
REMOTE_DIR="${FN_KNOCK_REMOTE_DIR:-/tmp/fn-knock-fpk}"
APP_NAME="${FN_KNOCK_APP_NAME:-fn-knock}"
APP_VERSION="$(fn_knock_app_version "${ROOT_DIR}")"
LOCAL_APP_DIR="${FN_KNOCK_LOCAL_APP_DIR:-apps/fn-knock}"
LOCAL_FPK_PATH="${FN_KNOCK_LOCAL_FPK_PATH:-apps/fn-knock/dist/fn-knock.fpk}"
REMOTE_SOURCE_DIR="${REMOTE_DIR}/src"
REMOTE_BUILD_AMD64_DIR="${REMOTE_DIR}/build-amd64"
REMOTE_BUILD_ARM64_DIR="${REMOTE_DIR}/build-arm64"
REMOTE_FPK_AMD64_PATH="${REMOTE_DIR}/${APP_NAME}-amd64.fpk"
REMOTE_FPK_ARM64_PATH="${REMOTE_DIR}/${APP_NAME}-arm64.fpk"
REMOTE_UI_INDEX="/usr/local/apps/@appcenter/${APP_NAME}/ui/index.cgi"
REMOTE_LOG_FILE="/usr/local/apps/@appdata/${APP_NAME}/info.log"
REMOTE_INSTALL_ENV_PATH="${REMOTE_DIR}/install.env"
REMOTE_APPCENTER_TMP_DIR="${FN_KNOCK_REMOTE_APPCENTER_TMP_DIR:-/tmp/appcenter}"
WIZARD_ADMIN_VIEW_PORT="${FN_KNOCK_WIZARD_ADMIN_VIEW_PORT:-7991}"
WIZARD_BACKEND_PORT="${FN_KNOCK_WIZARD_BACKEND_PORT:-7998}"
WIZARD_AUTH_PORT="${FN_KNOCK_WIZARD_AUTH_PORT:-7997}"
WIZARD_GO_BACKEND_PORT="${FN_KNOCK_WIZARD_GO_BACKEND_PORT:-7996}"
WIZARD_GO_REPROXY_PORT="${FN_KNOCK_WIZARD_GO_REPROXY_PORT:-7999}"

log() {
  echo "[fn-knock-deploy] $*"
}

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
FPK_ARCHES=()

get_remote_status() {
  ssh "${REMOTE_HOST}" "appcenter-cli status '${APP_NAME}' 2>/dev/null || true"
}

resolve_remote_install_volume() {
  local configured_volume="${FN_KNOCK_REMOTE_INSTALL_VOLUME:-}"

  if [ -n "${configured_volume}" ]; then
    case "${configured_volume}" in
      0|*[!0-9]*)
        echo "ERROR: FN_KNOCK_REMOTE_INSTALL_VOLUME must be a positive volume index" >&2
        return 1
        ;;
    esac

    echo "${configured_volume}"
    return 0
  fi

  ssh "${REMOTE_HOST}" '
    for dir in /vol[1-9]*; do
      [ -d "${dir}/@appcenter" ] || continue
      volume="${dir#/vol}"
      case "${volume}" in
        ""|*[!0-9]*)
          continue
          ;;
      esac
      echo "${volume}"
      exit 0
    done
  '
}

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
        echo "ERROR: invalid FPK architecture: ${arch}; expected amd64/x86 or arm64" >&2
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
    echo "ERROR: FPK architecture list is empty" >&2
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

assert_remote_installed() {
  local status
  status="$(get_remote_status)"
  echo "${status}"
  if echo "${status}" | grep -qi "noinstall"; then
    echo "ERROR: application '${APP_NAME}' is not installed on remote host" >&2
    exit 1
  fi
}

wait_for_remote_running() {
  local timeout="${FN_KNOCK_REMOTE_START_TIMEOUT:-60}"
  local started_at
  local status

  case "${timeout}" in
    0|*[!0-9]*)
      echo "ERROR: FN_KNOCK_REMOTE_START_TIMEOUT must be a positive number of seconds" >&2
      return 1
      ;;
  esac

  started_at="$(date +%s)"
  while true; do
    status="$(get_remote_status)"
    echo "${status}"
    if echo "${status}" | grep -qi "running"; then
      return 0
    fi
    if echo "${status}" | grep -qi "noinstall"; then
      echo "ERROR: application '${APP_NAME}' is not installed on remote host" >&2
      return 1
    fi
    if [ "$(( $(date +%s) - started_at ))" -ge "${timeout}" ]; then
      echo "ERROR: application '${APP_NAME}' did not reach running state within ${timeout}s" >&2
      return 1
    fi
    sleep 2
  done
}

resolve_remote_ui_index() {
  ssh "${REMOTE_HOST}" "for p in '${REMOTE_UI_INDEX}' '/usr/local/apps/@appcenter/${APP_NAME}/app/ui/index.cgi'; do if [ -f \"\$p\" ]; then echo \"\$p\"; exit 0; fi; done; exit 1"
}

resolve_remote_www_index() {
  ssh "${REMOTE_HOST}" "for p in '/usr/local/apps/@appcenter/${APP_NAME}/ui/www/index.html' '/usr/local/apps/@appcenter/${APP_NAME}/app/ui/www/index.html'; do if [ -f \"\$p\" ]; then echo \"\$p\"; exit 0; fi; done; exit 1"
}

resolve_deploy_rust_builder() {
  local builder="${FN_KNOCK_FPK_RUST_BUILDER:-auto}"

  case "${builder}" in
    "")
      echo "auto"
      ;;
    auto|zig|docker)
      echo "${builder}"
      ;;
    *)
      echo "ERROR: unsupported FN_KNOCK_FPK_RUST_BUILDER=${builder}; expected auto, zig, or docker" >&2
      exit 1
      ;;
  esac
}

run_local_package() {
  local rust_builder
  local build_script="${LOCAL_APP_DIR}/scripts/build-package.sh"

  rust_builder="$(resolve_deploy_rust_builder)"
  if [ ! -x "${build_script}" ]; then
    echo "ERROR: local package build script is missing or not executable: ${build_script}" >&2
    exit 1
  fi

  log "Step 1/4: Build package assets locally (${FPK_ARCHES[*]}, Rust builder: ${rust_builder})"
  FN_KNOCK_FPK_RUST_BUILDER="${rust_builder}" "${build_script}"
}

run_remote_pack_for_arch() {
  local arch="$1"
  local build_dir="$2"
  local output_path="$3"

  log "Step 2/4: Build ${arch} FPK on remote host"
  ssh "${REMOTE_HOST}" bash -s -- "${REMOTE_SOURCE_DIR}" "${build_dir}" "${output_path}" "${APP_NAME}" "${arch}" <<'EOF'
set -euo pipefail

source_dir="$1"
build_dir="$2"
output_path="$3"
app_name="$4"
arch="$5"

gateway_bins=(
  "go-reauth-proxy-linux-amd64"
  "go-reauth-proxy-linux-arm64"
  "go-reauth-proxy-linux-arm"
)

case "${arch}" in
  amd64)
    keep_bin="go-reauth-proxy-linux-amd64"
    keep_rust_bin="server-admin-rs-linux-amd64"
    install_dep_apps=""
    manifest_platform="x86"
    ;;
  arm64)
    keep_bin="go-reauth-proxy-linux-arm64"
    keep_rust_bin="server-admin-rs-linux-arm64"
    install_dep_apps=""
    manifest_platform="arm"
    ;;
  *)
    echo "[remote-fn-knock] unsupported arch: ${arch}" >&2
    exit 1
    ;;
esac

rm -rf "${build_dir}"
mkdir -p "${build_dir}"
rsync -a --delete "${source_dir}/" "${build_dir}/"

# Never ship Finder/AppleDouble metadata that may exist in a developer's
# working copy. Besides being noise, fnpack would otherwise place it inside
# app.tgz where it cannot be removed after installation.
find "${build_dir}" -type f \( -name '.DS_Store' -o -name '._*' \) -delete

# fnpack wraps app/ in a gzip archive, so gzip sidecars barely compress again.
# Retain Brotli sidecars plus the original files for transparent fallback.
find \
  "${build_dir}/app/ui/www" \
  "${build_dir}/app/server-auth-view/dist" \
  -type f -name '*.gz' -delete

for bin in "${gateway_bins[@]}"; do
  if [ "${bin}" != "${keep_bin}" ]; then
    rm -f "${build_dir}/app/server/${bin}"
  fi
done

chmod +x "${build_dir}/app/server/${keep_bin}" 2>/dev/null || true

if [ ! -x "${build_dir}/app/server/${keep_rust_bin}" ]; then
  echo "[remote-fn-knock] missing Rust backend for ${arch}: app/server/${keep_rust_bin}" >&2
  exit 1
fi
cp -f "${build_dir}/app/server/${keep_rust_bin}" "${build_dir}/app/server/server-admin-rs"
rm -f "${build_dir}"/app/server/server-admin-rs-linux-*
chmod +x "${build_dir}/app/server/server-admin-rs"

manifest_file="${build_dir}/manifest"
tmp_manifest="$(mktemp)"
awk -v dep_apps="${install_dep_apps}" -v platform="${manifest_platform}" '
  BEGIN { updated = 0 }
  /^platform=/ {
    print "platform=" platform
    next
  }
  /^install_dep_apps=/ {
    print "install_dep_apps=" dep_apps
    updated = 1
    next
  }
  { print }
  END {
    if (!updated) {
      print "install_dep_apps=" dep_apps
    }
  }
' "${manifest_file}" > "${tmp_manifest}"
mv "${tmp_manifest}" "${manifest_file}"

cd "${build_dir}"
rm -f "${app_name}.fpk"
fnpack build -d .
mv -f "${app_name}.fpk" "${output_path}"
echo "[remote-fn-knock] built ${arch} package -> ${output_path}"
EOF
}

verify_fpk_payload() {
  local fpk_path="$1"
  local keep_bin="$2"
  local rust_arch="$3"
  local app_listing
  local normalized_listing
  local bin
  local rust_tmp
  local rust_file_info

  if ! app_listing="$(tar -xOzf "${fpk_path}" app.tgz | tar -tzf -)"; then
    echo "ERROR: failed to inspect FPK app payload: ${fpk_path}" >&2
    exit 1
  fi

  normalized_listing="$(printf '%s\n' "${app_listing}" | sed 's#^\./##')"
  if printf '%s\n' "${normalized_listing}" | grep -Eq '(^|/)\.DS_Store$|(^|/)\._'; then
    echo "ERROR: FPK ${fpk_path} contains macOS metadata files" >&2
    exit 1
  fi
  if ! printf '%s\n' "${normalized_listing}" | \
    grep -Eq "^ui/www/assets/v${APP_VERSION//./\\.}/[^/]+$"; then
    echo "ERROR: FPK ${fpk_path} is missing the versioned admin asset namespace for ${APP_VERSION}" >&2
    exit 1
  fi
  if ! printf '%s\n' "${normalized_listing}" | grep -Fxq "server/${keep_bin}"; then
    echo "ERROR: FPK ${fpk_path} is missing expected gateway binary: ${keep_bin}" >&2
    exit 1
  fi

  for bin in \
    go-reauth-proxy-linux-amd64 \
    go-reauth-proxy-linux-arm64 \
    go-reauth-proxy-linux-arm
  do
    if [ "${bin}" != "${keep_bin}" ] && printf '%s\n' "${normalized_listing}" | grep -Fxq "server/${bin}"; then
      echo "ERROR: FPK ${fpk_path} contains non-target gateway binary: ${bin}" >&2
      exit 1
    fi
  done

  if ! printf '%s\n' "${normalized_listing}" | grep -Fxq "server/server-admin-rs"; then
    echo "ERROR: FPK ${fpk_path} is missing Rust backend: server/server-admin-rs" >&2
    exit 1
  fi
  if printf '%s\n' "${normalized_listing}" | grep -Eq '^server/server-admin-rs-linux-'; then
    echo "ERROR: FPK ${fpk_path} contains staging Rust backend binaries" >&2
    exit 1
  fi

  rust_tmp="$(mktemp)"
  if ! tar -xOzf "${fpk_path}" app.tgz | tar -xOzf - server/server-admin-rs > "${rust_tmp}" 2>/dev/null; then
    rm -f "${rust_tmp}"
    echo "ERROR: failed to extract Rust backend from FPK: ${fpk_path}" >&2
    exit 1
  fi
  rust_file_info="$(file -b "${rust_tmp}")"
  rm -f "${rust_tmp}"
  case "${rust_arch}" in
    amd64)
      if ! printf '%s\n' "${rust_file_info}" | grep -Eq 'ELF 64-bit LSB.*x86-64'; then
        echo "ERROR: FPK ${fpk_path} Rust backend is not Linux x86-64 ELF: ${rust_file_info}" >&2
        exit 1
      fi
      ;;
    arm64)
      if ! printf '%s\n' "${rust_file_info}" | grep -Eq 'ELF 64-bit LSB.*(ARM aarch64|aarch64)'; then
        echo "ERROR: FPK ${fpk_path} Rust backend is not Linux arm64 ELF: ${rust_file_info}" >&2
        exit 1
      fi
      ;;
    *)
      echo "ERROR: unsupported Rust backend arch verifier: ${rust_arch}" >&2
      exit 1
      ;;
  esac
}

run_remote_pack() {
  log "Step 2/4: Upload app sources to remote fnpack directory"
  ssh "${REMOTE_HOST}" "mkdir -p '${REMOTE_DIR}' '${REMOTE_SOURCE_DIR}'"
  rsync -az --delete --delete-excluded \
    --exclude '.DS_Store' \
    --exclude '._*' \
    "${LOCAL_APP_DIR}/" "${REMOTE_HOST}:${REMOTE_SOURCE_DIR}/"

  if fpk_arch_enabled "amd64"; then
    run_remote_pack_for_arch "amd64" "${REMOTE_BUILD_AMD64_DIR}" "${REMOTE_FPK_AMD64_PATH}"
  fi
  if fpk_arch_enabled "arm64"; then
    run_remote_pack_for_arch "arm64" "${REMOTE_BUILD_ARM64_DIR}" "${REMOTE_FPK_ARM64_PATH}"
  fi

  log "Step 2/4: Pull generated FPKs back to local workspace"
  mkdir -p "$(dirname "${LOCAL_FPK_AMD64_PATH}")"
  if fpk_arch_enabled "amd64"; then
    scp "${REMOTE_HOST}:${REMOTE_FPK_AMD64_PATH}" "${LOCAL_FPK_AMD64_PATH}"
    verify_fpk_payload "${LOCAL_FPK_AMD64_PATH}" "go-reauth-proxy-linux-amd64" "amd64"
  fi
  if fpk_arch_enabled "arm64"; then
    scp "${REMOTE_HOST}:${REMOTE_FPK_ARM64_PATH}" "${LOCAL_FPK_ARM64_PATH}"
    verify_fpk_payload "${LOCAL_FPK_ARM64_PATH}" "go-reauth-proxy-linux-arm64" "arm64"
  fi
}

run_remote_install() {
  local install_command
  local install_volume
  local status

  if ! fpk_arch_enabled "amd64"; then
    echo "ERROR: install-remote installs the x86/amd64 FPK; include amd64 in FN_KNOCK_FPK_ARCHES" >&2
    exit 1
  fi

  log "Step 3/4: Stop and uninstall old app version"
  ssh "${REMOTE_HOST}" "appcenter-cli stop '${APP_NAME}' || true"
  ssh "${REMOTE_HOST}" "appcenter-cli uninstall '${APP_NAME}' || true"

  log "Step 3/4: Prepare wizard env file for CLI installation"
  ssh "${REMOTE_HOST}" "cat > '${REMOTE_INSTALL_ENV_PATH}' <<'EOF'
wizard_admin_view_port=${WIZARD_ADMIN_VIEW_PORT}
wizard_backend_port=${WIZARD_BACKEND_PORT}
wizard_auth_port=${WIZARD_AUTH_PORT}
wizard_go_backend_port=${WIZARD_GO_BACKEND_PORT}
wizard_go_reproxy_port=${WIZARD_GO_REPROXY_PORT}
EOF"

  log "Step 3/4: Ensure appcenter temp directory exists"
  ssh "${REMOTE_HOST}" "mkdir -p '${REMOTE_APPCENTER_TMP_DIR}'"

  install_volume="$(resolve_remote_install_volume)"
  log "Step 3/4: Install and start new amd64 FPK"
  if [ -n "${install_volume}" ]; then
    log "Step 3/4: Use appcenter volume ${install_volume}"
    install_command="appcenter-cli install-fpk '${REMOTE_FPK_AMD64_PATH}' --env '${REMOTE_INSTALL_ENV_PATH}' --volume '${install_volume}'"
  else
    log "Step 3/4: No appcenter volume detected; use the remote CLI default"
    install_command="appcenter-cli install-fpk '${REMOTE_FPK_AMD64_PATH}' --env '${REMOTE_INSTALL_ENV_PATH}'"
  fi
  if ! ssh "${REMOTE_HOST}" "${install_command}"; then
    log "Step 3/4: Install failed, tailing appcenter error log for diagnostics"
    ssh "${REMOTE_HOST}" "tail -n 120 /var/log/trim_app_center/error.log || true"
    exit 1
  fi
  log "Step 3/4: Verify installation state"
  assert_remote_installed
  status="$(get_remote_status)"
  if echo "${status}" | grep -Eqi "starting|running"; then
    log "Step 3/4: Appcenter already reports ${status}; skip duplicate start"
  else
    ssh "${REMOTE_HOST}" "appcenter-cli start '${APP_NAME}'"
  fi
  log "Step 3/4: Verify runtime state"
  wait_for_remote_running

  log "Step 3/4: Tail runtime log"
  ssh "${REMOTE_HOST}" "tail -n 200 '${REMOTE_LOG_FILE}' || true"
}

run_remote_verify() {
  assert_remote_installed >/dev/null
  log "Step 4/4: Verify installed index.cgi hash"
  local local_hash
  local remote_hash
  local remote_ui_index
  local_hash="$(shasum -a 256 "${LOCAL_APP_DIR}/app/ui/index.cgi" | awk '{print $1}')"
  remote_ui_index="$(resolve_remote_ui_index)" || {
    echo "ERROR: unable to locate remote index.cgi for '${APP_NAME}'" >&2
    exit 1
  }
  remote_hash="$(ssh "${REMOTE_HOST}" "shasum -a 256 '${remote_ui_index}' | awk '{print \$1}'")"
  echo "local index.cgi  sha256: ${local_hash}"
  echo "remote index.cgi sha256: ${remote_hash}"
  echo "remote index.cgi path: ${remote_ui_index}"

  if [ "${local_hash}" != "${remote_hash}" ]; then
    echo "ERROR: installed index.cgi does not match local package file" >&2
    exit 1
  fi

  log "Step 4/4: Verify installed ui/www/index.html hash"
  local local_www_index
  local remote_www_index
  local local_www_hash
  local remote_www_hash
  local_www_index="${LOCAL_APP_DIR}/app/ui/www/index.html"
  remote_www_index="$(resolve_remote_www_index)" || {
    echo "ERROR: unable to locate remote ui/www/index.html for '${APP_NAME}'" >&2
    exit 1
  }
  local_www_hash="$(shasum -a 256 "${local_www_index}" | awk '{print $1}')"
  remote_www_hash="$(ssh "${REMOTE_HOST}" "shasum -a 256 '${remote_www_index}' | awk '{print \$1}'")"
  echo "local index.html  sha256: ${local_www_hash}"
  echo "remote index.html sha256: ${remote_www_hash}"
  echo "remote index.html path: ${remote_www_index}"

  if [ "${local_www_hash}" != "${remote_www_hash}" ]; then
    echo "ERROR: installed ui/www/index.html does not match local package file" >&2
    exit 1
  fi

  log "Step 4/4: Verify installed SSLSettings assets"
  local local_assets_dir
  local remote_assets_dir
  local local_ssl_assets
  local remote_ssl_assets
  local asset_name
  local asset_local_hash
  local asset_remote_hash
  local_assets_dir="${LOCAL_APP_DIR}/app/ui/www/assets"
  remote_assets_dir="$(dirname "${remote_www_index}")/assets"
  local_ssl_assets="$(find "${local_assets_dir}" -maxdepth 1 -type f -name 'SSLSettings-*.js' | sed 's|.*/||' | sort)"
  remote_ssl_assets="$(ssh "${REMOTE_HOST}" "find '${remote_assets_dir}' -maxdepth 1 -type f -name 'SSLSettings-*.js' | sed 's|.*/||' | sort")"
  echo "local SSLSettings assets:"
  printf '%s\n' "${local_ssl_assets}"
  echo "remote SSLSettings assets:"
  printf '%s\n' "${remote_ssl_assets}"

  if [ "${local_ssl_assets}" != "${remote_ssl_assets}" ]; then
    echo "ERROR: installed SSLSettings assets do not match local package files" >&2
    exit 1
  fi

  while IFS= read -r asset_name; do
    [ -n "${asset_name}" ] || continue
    asset_local_hash="$(shasum -a 256 "${local_assets_dir}/${asset_name}" | awk '{print $1}')"
    asset_remote_hash="$(ssh "${REMOTE_HOST}" "shasum -a 256 '${remote_assets_dir}/${asset_name}' | awk '{print \$1}'")"
    echo "local ${asset_name}  sha256: ${asset_local_hash}"
    echo "remote ${asset_name} sha256: ${asset_remote_hash}"
    if [ "${asset_local_hash}" != "${asset_remote_hash}" ]; then
      echo "ERROR: installed ${asset_name} does not match local package file" >&2
      exit 1
    fi
  done <<EOF
${local_ssl_assets}
EOF

  log "Step 4/4: Show key section from remote index.cgi"
  ssh "${REMOTE_HOST}" "sed -n '170,280p' '${remote_ui_index}'"
}

usage() {
  cat <<'EOF'
Usage:
  bash ./scripts/fn-knock-deploy.sh <command>

Commands:
  pack-remote     Run local package build + remote fnpack build + download generated FPKs
  install-remote  Install/start amd64 FPK on remote host and print runtime logs
  verify-remote   Verify installed index.cgi hash and print key lines
  deploy          Run all steps in order (pack-remote -> install-remote -> verify-remote)

Optional env overrides:
  FN_KNOCK_REMOTE_HOST  (default: root@192.168.31.98)
  FN_KNOCK_REMOTE_DIR   (default: /tmp/fn-knock-fpk)
  FN_KNOCK_REMOTE_INSTALL_VOLUME (optional positive volume index; auto-detected from /volN/@appcenter)
  FN_KNOCK_REMOTE_START_TIMEOUT (default: 60 seconds)
  FN_KNOCK_APP_NAME     (default: fn-knock)
  FN_KNOCK_LOCAL_APP_DIR (default: apps/fn-knock)
  FN_KNOCK_LOCAL_FPK_PATH (default: apps/fn-knock/dist/fn-knock.fpk; downloads as -amd64/-arm64)
  FN_KNOCK_FPK_ARCHES (space/comma list: amd64/x86 and/or arm64; default: amd64 arm64)
  FN_KNOCK_WIZARD_ADMIN_VIEW_PORT (default: 7991)
  FN_KNOCK_WIZARD_BACKEND_PORT (default: 7998)
  FN_KNOCK_WIZARD_AUTH_PORT (default: 7997)
  FN_KNOCK_WIZARD_GO_BACKEND_PORT (default: 7996)
  FN_KNOCK_WIZARD_GO_REPROXY_PORT (default: 7999)
  FN_KNOCK_FPK_RUST_BUILDER (auto|zig|docker; default: auto)
  FN_KNOCK_RUST_PARALLEL_RELEASE (set 1 to use thin LTO + multi codegen units for faster builds)
  CARGO_BUILD_JOBS (Cargo job count; defaults to CPU count when FN_KNOCK_RUST_PARALLEL_RELEASE=1)
  CARGO_PROFILE_RELEASE_LTO (optional Cargo release LTO override, e.g. thin)
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS (optional release codegen units override)
EOF
}

read_fpk_arches

cmd="${1:-}"
case "${cmd}" in
  pack-remote)
    run_local_package
    run_remote_pack
    ;;
  install-remote)
    run_remote_install
    ;;
  verify-remote)
    run_remote_verify
    ;;
  deploy)
    run_local_package
    run_remote_pack
    run_remote_install
    run_remote_verify
    log "Completed deployment."
    ;;
  *)
    usage
    exit 1
    ;;
esac
