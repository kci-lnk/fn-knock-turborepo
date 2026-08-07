#!/bin/bash
set -euo pipefail

# Guard: no panic!/todo!/unimplemented! in production Rust code.
# Test code (files named tests.rs or code under #[cfg(test)] / mod tests / #[test])
# is skipped; only request-path and runtime code is checked.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIR="${ROOT_DIR}/apps/server-admin-rs/src"

failures=0

while IFS= read -r file; do
  if [[ "$(basename "$file")" == "tests.rs" ]]; then
    continue
  fi
  in_test=0
  line_no=0
  while IFS= read -r line; do
    line_no=$((line_no + 1))
    if [[ "$line" =~ '#[cfg(test)]' || "$line" =~ '#[test]' || "$line" =~ ^[[:space:]]*mod[[:space:]]+tests ]]; then
      in_test=1
    fi
    if [[ "$in_test" == "0" && "$line" =~ panic!|todo!|unimplemented! ]]; then
      trimmed="${line#"${line%%[![:space:]]*}"}"
      case "$trimmed" in
        //* | \** | '/*'*) continue ;;
      esac
      echo "production panic candidate: ${file}:${line_no}: ${line}"
      failures=$((failures + 1))
    fi
  done < "$file"
done < <(rg --files "$SRC_DIR" -g '*.rs')

if [[ "$failures" -gt 0 ]]; then
  echo "rust:panic-guard failed: ${failures} production panic candidate(s) found"
  exit 1
fi

echo "rust:panic-guard ok: no panic!/todo!/unimplemented! in production code"
