#!/bin/bash
set -euo pipefail

# Removes regenerable build artifacts: Cargo targets, Turbo cache and the
# root dist/ staging area. Everything deleted here is produced again by
# the build/package scripts; node_modules is intentionally left untouched.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

log() {
  echo "[fn-knock-clean] $*"
}

clean_cargo_target() {
  local manifest="$1"
  local target_dir
  target_dir="$(cd "$(dirname "${manifest}")" && pwd)/target"
  if [ ! -d "${target_dir}" ]; then
    return 0
  fi
  log "cleaning $(du -sh "${target_dir}" | cut -f1) in ${target_dir}"
  if command -v cargo >/dev/null 2>&1; then
    cargo clean --manifest-path "${manifest}" >/dev/null 2>&1 || rm -rf "${target_dir}"
  else
    rm -rf "${target_dir}"
  fi
}

clean_cargo_target "${ROOT_DIR}/apps/server-admin-rs/Cargo.toml"
clean_cargo_target "${ROOT_DIR}/apps/fn-knock-desktop/native/Cargo.toml"

if [ -d "${ROOT_DIR}/.turbo/cache" ]; then
  log "cleaning Turbo cache ($(du -sh "${ROOT_DIR}/.turbo/cache" | cut -f1))"
  rm -rf "${ROOT_DIR}/.turbo/cache"
fi

if [ -d "${ROOT_DIR}/dist" ]; then
  log "cleaning dist/* ($(du -sh "${ROOT_DIR}/dist" | cut -f1))"
  find "${ROOT_DIR}/dist" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
fi

log "done"
