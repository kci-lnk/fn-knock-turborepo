#!/bin/bash
set -euo pipefail

# Compile the production library/binary targets with Clippy's macro-specific
# lints. Rust evaluates #[cfg(test)] itself, so test-only panic sites are
# excluded without trying to approximate Rust scopes in shell.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST_PATH="${FN_KNOCK_RUST_PANIC_GUARD_MANIFEST:-${ROOT_DIR}/apps/server-admin-rs/Cargo.toml}"
CARGO_BIN="${CARGO:-cargo}"

fail() {
  printf '[rust:panic-guard] ERROR: %s\n' "$*" >&2
  exit 1
}

[ -f "${MANIFEST_PATH}" ] || fail "Cargo manifest not found: ${MANIFEST_PATH}"
command -v "${CARGO_BIN}" >/dev/null 2>&1 || fail "cargo not found: ${CARGO_BIN}"
"${CARGO_BIN}" clippy --version >/dev/null 2>&1 || fail "Clippy is required; install it with: rustup component add clippy"

manifest_dir="$(cd "$(dirname "${MANIFEST_PATH}")" && pwd)"
cargo_args=(
  clippy
  --manifest-path "${MANIFEST_PATH}"
  --lib
  --bins
)
if [ -f "${manifest_dir}/Cargo.lock" ]; then
  cargo_args+=(--locked)
fi

printf '[rust:panic-guard] checking production Rust targets with Clippy\n'
"${CARGO_BIN}" "${cargo_args[@]}" -- \
  -D clippy::panic \
  -D clippy::todo \
  -D clippy::unimplemented
printf '[rust:panic-guard] ok: production targets contain no panic!/todo!/unimplemented!\n'
