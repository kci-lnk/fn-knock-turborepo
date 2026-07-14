#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNNER="${FN_KNOCK_SCRIPT_RUNNER:-bun}"
START_TS="$(date +%s)"

cd "${ROOT_DIR}"

log() {
  echo "[fn-knock-deploy-all] $*"
}

format_duration() {
  local total_seconds="$1"
  local minutes=$((total_seconds / 60))
  local seconds=$((total_seconds % 60))

  printf '%d分%02d秒' "${minutes}" "${seconds}"
}

finish() {
  local exit_code="$?"
  local end_ts
  local elapsed

  trap - EXIT
  end_ts="$(date +%s)"
  elapsed=$((end_ts - START_TS))

  if [ "${exit_code}" -eq 0 ]; then
    log "全部完成，总耗时: $(format_duration "${elapsed}")"
  else
    log "执行失败(exit ${exit_code})，总耗时: $(format_duration "${elapsed}")"
  fi

  exit "${exit_code}"
}

run_package_script() {
  local script_name="$1"

  log "Running: ${RUNNER} run ${script_name}"
  "${RUNNER}" run "${script_name}"
}

verify_package_outputs() {
  local artifacts_dir="${FN_KNOCK_ARTIFACTS_DIR:-${ROOT_DIR}/dist/fn-knock-artifacts}"
  local linux_dir="${FN_KNOCK_PREPARED_LINUX_DIR:-${artifacts_dir}/linux}"
  local -a linux_packages
  local -a spk_packages

  shopt -s nullglob
  linux_packages=("${linux_dir}"/fn-knock-linux-*.tar.gz)
  if [ -n "${FN_KNOCK_SYNOLOGY_OUTPUT:-}" ]; then
    spk_packages=("${FN_KNOCK_SYNOLOGY_OUTPUT}")
  else
    spk_packages=("${ROOT_DIR}"/dist/synology/*.spk)
  fi
  shopt -u nullglob

  if [ "${#linux_packages[@]}" -eq 0 ]; then
    log "未找到 Linux 安装包: ${linux_dir}/fn-knock-linux-*.tar.gz"
    return 1
  fi
  if [ "${#spk_packages[@]}" -eq 0 ] || [ ! -f "${spk_packages[0]}" ]; then
    log "未找到 Synology SPK 安装包"
    return 1
  fi

  log "本地安装包已就绪: Linux ${#linux_packages[@]} 个，SPK ${#spk_packages[@]} 个"
}

trap finish EXIT

log "准备共享构建产物（包含 Linux 安装包）"
run_package_script "fn-knock:prepare-artifacts"

export FN_KNOCK_ARTIFACTS_ALREADY_PREPARED=1

run_package_script "fn-knock:deploy"
run_package_script "fn-knock:openwrt:build"
run_package_script "fn-knock:docker:local-deploy"

log "使用共享产物打包 Synology SPK"
run_package_script "fn-knock:spk:build:prepared"
verify_package_outputs
