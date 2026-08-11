#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTER="${ROOT_DIR}/apps/server-admin-rs/src/app/router.rs"
ADMIN_CLIENT="${ROOT_DIR}/apps/server-admin-view/src/lib/api/client.ts"

fail() {
  printf '[test-cgi-origin-forwarding] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local file="$1" expected="$2" label="$3"
  grep -Fq -- "${expected}" "${file}" || fail "${label}: ${file} is missing ${expected}"
}

for cgi in \
  "${ROOT_DIR}/apps/fn-knock/app/ui/index.cgi" \
  "${ROOT_DIR}/apps/fn-knock-lite/app/ui/index.cgi" \
  "${ROOT_DIR}/apps/fn-knock-synology/package/ui/index.cgi"
do
  sh -n "${cgi}" || fail "invalid CGI shell syntax: ${cgi}"
  assert_contains "${cgi}" 'HTTP_HOST' 'external CGI authority forwarding'
  assert_contains "${cgi}" '-H "host:' 'loopback request Host override'
  assert_contains "${cgi}" 'HTTP_ORIGIN' 'browser Origin forwarding'
  assert_contains "${cgi}" 'HTTP_SEC_FETCH_SITE' 'Fetch Metadata forwarding'
  assert_contains "${cgi}" '-H "sec-fetch-site:' 'Fetch Metadata header'
  assert_contains "${cgi}" 'HTTP_X_FN_KNOCK_BROWSER_ORIGIN' 'CGI browser-origin proof forwarding'
  assert_contains "${cgi}" '-H "x-fn-knock-browser-origin:' 'CGI browser-origin proof header'
  assert_contains "${cgi}" 'PUBLIC_SCHEME' 'external CGI scheme resolution'
  assert_contains "${cgi}" '-H "x-forwarded-proto:' 'external CGI scheme forwarding'
done

assert_contains "${ROUTER}" 'eq_ignore_ascii_case("cross-site")' 'cross-site mutation rejection'
assert_contains "${ROUTER}" 'origin.port_or_known_default() == expected.port_or_known_default()' 'origin authority comparison'
assert_contains "${ROUTER}" 'x-fn-knock-browser-origin' 'loopback CGI browser-origin proof validation'
assert_contains "${ADMIN_CLIENT}" 'X-Fn-Knock-Browser-Origin' 'same-origin frontend proof header'

printf '[test-cgi-origin-forwarding] CGI authority forwarding contract passed\n'
