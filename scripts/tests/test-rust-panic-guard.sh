#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GUARD="${ROOT_DIR}/scripts/check-rust-prod-panics.sh"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-panic-guard.XXXXXX")"
FIXTURE="${WORK_DIR}/fixture"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-rust-panic-guard] ERROR: %s\n' "$*" >&2
  exit 1
}

mkdir -p "${FIXTURE}/src"
cat > "${FIXTURE}/Cargo.toml" <<'EOF'
[package]
name = "panic-guard-fixture"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"
EOF

cat > "${FIXTURE}/src/lib.rs" <<'EOF'
#[cfg(test)]
mod tests {
    #[test]
    fn test_panics_are_allowed() {
        panic!("test-only panic");
    }
}

pub fn production_code_after_test_module() -> bool {
    true
}
EOF

if ! FN_KNOCK_RUST_PANIC_GUARD_MANIFEST="${FIXTURE}/Cargo.toml" \
  bash "${GUARD}" >"${WORK_DIR}/allowed.log" 2>&1; then
  cat "${WORK_DIR}/allowed.log" >&2
  fail "test-only panic was rejected"
fi

cat > "${FIXTURE}/src/lib.rs" <<'EOF'
#[cfg(test)]
mod tests {
    #[test]
    fn test_panics_are_allowed() {
        panic!("test-only panic");
    }
}

pub fn production_panic_after_test_module() {
    panic!("production panic");
}

pub fn unfinished_production_code() {
    todo!("production todo");
}

pub fn unimplemented_production_code() {
    unimplemented!("production placeholder");
}
EOF

if FN_KNOCK_RUST_PANIC_GUARD_MANIFEST="${FIXTURE}/Cargo.toml" \
  bash "${GUARD}" >"${WORK_DIR}/rejected.log" 2>&1; then
  fail "production macros after a test module were not rejected"
fi
for lint in panic todo unimplemented; do
  grep -Fq "clippy::${lint}" "${WORK_DIR}/rejected.log" || {
    cat "${WORK_DIR}/rejected.log" >&2
    fail "missing clippy::${lint} diagnostic"
  }
done

printf '[test-rust-panic-guard] cfg(test) exclusion and production rejection passed\n'
