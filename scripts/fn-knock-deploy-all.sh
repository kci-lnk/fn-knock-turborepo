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

trap finish EXIT

run_package_script "fn-knock:prepare-artifacts"

export FN_KNOCK_ARTIFACTS_ALREADY_PREPARED=1

run_package_script "fn-knock:deploy"
run_package_script "fn-knock:openwrt:build"
run_package_script "fn-knock:docker:local-deploy"
