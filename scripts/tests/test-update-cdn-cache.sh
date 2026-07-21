#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

fail() {
  printf '[test-update-cdn-cache] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_absent() {
  local file="$1" pattern="$2" description="$3"
  if grep -Fq -- "${pattern}" "${ROOT_DIR}/${file}"; then
    fail "${description}: ${file} contains '${pattern}'"
  fi
}

assert_present() {
  local file="$1" pattern="$2" description="$3"
  grep -Fq -- "${pattern}" "${ROOT_DIR}/${file}" || \
    fail "${description}: ${file} is missing '${pattern}'"
}

DESKTOP_UPDATE="apps/fn-knock-desktop/native/src/update.rs"
SERVER_UPDATE="apps/server-admin-rs/src/system/update.rs"
WAF_RULES="apps/server-admin-rs/src/waf/routes/rules.rs"
LINUX_INSTALL="deploy/linux/install.sh"
LINUX_MANAGER="deploy/linux/knock"

assert_present "${DESKTOP_UPDATE}" '.get(ENDPOINT)' "desktop update checks must use the stable manifest URL"
assert_absent "${DESKTOP_UPDATE}" '{ENDPOINT}?t=' "desktop update checks must not cache-bust"
assert_absent "${DESKTOP_UPDATE}" 'reqwest::header::CACHE_CONTROL' "desktop update requests must allow CDN caching"

assert_present "${SERVER_UPDATE}" '.get(OTA_LATEST_URL)' "server update checks must use the stable manifest URL"
assert_absent "${SERVER_UPDATE}" 'reqwest::header::CACHE_CONTROL' "server update requests and downloads must allow CDN caching"
assert_absent "${SERVER_UPDATE}" 'reqwest::header::PRAGMA' "server update requests and downloads must allow CDN caching"
assert_absent "${SERVER_UPDATE}" 'append_pair("t"' "server update checks must not cache-bust"

assert_present "${LINUX_MANAGER}" 'download_file "${base}/linux/latest/${arch}.env"' "Linux upgrades must use the stable manifest URL"
assert_absent "${LINUX_MANAGER}" 'cache_bust_url' "Linux upgrades must not cache-bust"
assert_absent "${LINUX_MANAGER}" 'Cache-Control: no-cache' "Linux upgrade downloads must allow CDN caching"
assert_absent "${LINUX_MANAGER}" 'Pragma: no-cache' "Linux upgrade downloads must allow CDN caching"

assert_present "${LINUX_INSTALL}" '"${BASE_URL%/}/linux/latest/${ARCH}.env"' "Linux installs must use the stable manifest URL"
assert_absent "${LINUX_INSTALL}" 'cache_bust_url' "Linux installs must not cache-bust"
assert_absent "${LINUX_INSTALL}" 'Cache-Control: no-cache' "Linux install downloads must allow CDN caching"
assert_absent "${LINUX_INSTALL}" 'Pragma: no-cache' "Linux install downloads must allow CDN caching"

assert_present "${WAF_RULES}" '.get(resolve_waf_url' "WAF downloads must use stable resolved URLs"
assert_absent "${WAF_RULES}" 'cache_busted_url' "WAF downloads must not cache-bust"
assert_absent "${WAF_RULES}" 'query_pairs_mut' "WAF downloads must not add query parameters"
assert_absent "${WAF_RULES}" '.header("cache-control"' "WAF downloads must allow CDN caching"
assert_absent "${WAF_RULES}" '.header("pragma"' "WAF downloads must allow CDN caching"

printf '[test-update-cdn-cache] all CDN cache request contracts passed\n'
