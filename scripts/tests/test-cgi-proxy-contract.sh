#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTER="${ROOT_DIR}/apps/server-admin-rs/src/app/router.rs"
ADMIN_CLIENT="${ROOT_DIR}/apps/server-admin-view/src/lib/api/client.ts"
FPK_CGI="${ROOT_DIR}/apps/fn-knock/app/ui/index.cgi"
GO_REPOSITORY="${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-cgi-compression.XXXXXX")"
trap 'rm -rf "${WORK_DIR}"' EXIT

fail() {
  printf '[test-cgi-proxy-contract] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local file="$1" expected="$2" label="$3"
  grep -Fq -- "${expected}" "${file}" || fail "${label}: ${file} is missing ${expected}"
}

assert_not_contains() {
  local file="$1" unexpected="$2" label="$3"
  if grep -Fq -- "${unexpected}" "${file}"; then
    fail "${label}: ${file} unexpectedly contains ${unexpected}"
  fi
}

for cgi in \
  "${ROOT_DIR}/apps/fn-knock/app/ui/index.cgi" \
  "${ROOT_DIR}/apps/fn-knock-lite/app/ui/index.cgi" \
  "${ROOT_DIR}/apps/fn-knock-synology/package/ui/index.cgi"
do
  sh -n "${cgi}" || fail "invalid CGI shell syntax: ${cgi}"
  assert_contains "${cgi}" 'HTTP_ORIGIN' 'browser Origin forwarding'
  assert_contains "${cgi}" 'Content-Length' 'response length forwarding'
  assert_not_contains "${cgi}" 'HTTP_HOST' 'retired CGI authority forwarding'
  assert_not_contains "${cgi}" 'PUBLIC_SCHEME' 'retired CGI scheme resolution'
  assert_not_contains "${cgi}" 'HTTP_SEC_FETCH_SITE' 'retired Fetch Metadata forwarding'
  assert_not_contains "${cgi}" 'HTTP_X_FN_KNOCK_BROWSER_ORIGIN' 'retired browser-origin proof'
  assert_not_contains "${cgi}" 's|src="/|src="./|g' 'compressed response body mutation'
done

for cgi in \
  "${ROOT_DIR}/apps/fn-knock/app/ui/index.cgi" \
  "${ROOT_DIR}/apps/fn-knock-lite/app/ui/index.cgi"
do
  assert_not_contains "${cgi}" 'HTTP_ACCEPT_ENCODING' 'fnOS WebView compression negotiation'
  assert_not_contains "${cgi}" '-H "accept-encoding:' 'fnOS WebView Accept-Encoding upstream header'
  assert_not_contains "${cgi}" 'emit_upstream_header "Content-Encoding"' 'fnOS WebView response encoding forwarding'
done

SYNOLOGY_CGI="${ROOT_DIR}/apps/fn-knock-synology/package/ui/index.cgi"
assert_contains "${SYNOLOGY_CGI}" 'HTTP_ACCEPT_ENCODING' 'Synology compression negotiation forwarding'
assert_contains "${SYNOLOGY_CGI}" '-H "accept-encoding:' 'Synology Accept-Encoding upstream header'
assert_contains "${SYNOLOGY_CGI}" 'Content-Encoding' 'Synology compressed response encoding forwarding'

assert_not_contains "${ROUTER}" 'same_origin_middleware' 'retired request-origin middleware'
assert_not_contains "${ROUTER}" 'browser_request_origin_allowed' 'retired request-origin filter'
assert_not_contains "${ADMIN_CLIENT}" 'X-Fn-Knock-Browser-Origin' 'retired browser-origin proof header'
assert_contains "${FPK_CGI}" 'TARGET_HOST=${ADMIN_TARGET_HOST:-"127.0.0.1"}' 'FPK loopback Rust target'

terminal_route_roots=("${ROOT_DIR}/apps" "${ROOT_DIR}/proto")
[ ! -d "${GO_REPOSITORY}" ] || terminal_route_roots+=("${GO_REPOSITORY}")
if rg -F '/api/admin/terminal/local' "${terminal_route_roots[@]}" \
  --glob '*.go' --glob '*.proto' >/dev/null 2>&1; then
  fail 'local terminal API must not be routed through the Go gateway or gRPC/proto'
fi

mkdir -p "${WORK_DIR}/bin"
cat > "${WORK_DIR}/bin/curl" <<'FAKE_CURL'
#!/bin/sh
header_file=""
body_file=""
accept_encoding=""
target_url=""
request_method="GET"
forward_body="false"
while [ "$#" -gt 0 ]; do
  case "$1" in
    -D)
      header_file="$2"
      shift 2
      ;;
    -o)
      body_file="$2"
      shift 2
      ;;
    -H)
      case "$2" in
        accept-encoding:*) accept_encoding="${2#accept-encoding: }" ;;
      esac
      shift 2
      ;;
    -X)
      request_method="$2"
      shift 2
      ;;
    --data-binary)
      forward_body="true"
      shift 2
      ;;
    *)
      target_url="$1"
      shift
      ;;
  esac
done
[ "${accept_encoding}" = "${EXPECTED_ACCEPT_ENCODING}" ] || exit 65
if [ "${forward_body}" = "true" ]; then
  forwarded_body="$(cat)"
else
  forwarded_body=""
fi
if [ -n "${CAPTURE_FILE:-}" ]; then
  {
    printf 'method=%s\n' "${request_method}"
    printf 'url=%s\n' "${target_url}"
    printf 'body=%s\n' "${forwarded_body}"
  } > "${CAPTURE_FILE}"
fi
body='raw-src="/fixture.js"'
case "${target_url}" in
  */assets/*) content_type='text/javascript; charset=utf-8' ;;
  *) content_type='text/html; charset=utf-8' ;;
esac
printf 'HTTP/1.1 200 OK\r\nContent-Type: %s\r\nCache-Control: public, max-age=31536000, immutable\r\nContent-Length: %s\r\nVary: Accept-Encoding\r\nX-Content-Type-Options: nosniff\r\n' \
  "${content_type}" "${#body}" > "${header_file}"
printf '\r\n' >> "${header_file}"
printf '%s' "${body}" > "${body_file}"
FAKE_CURL
chmod 755 "${WORK_DIR}/bin/curl"

for cgi in \
  "${ROOT_DIR}/apps/fn-knock/app/ui/index.cgi" \
  "${ROOT_DIR}/apps/fn-knock-lite/app/ui/index.cgi"
do
  output="$(
    PATH="${WORK_DIR}/bin:${PATH}" \
    EXPECTED_ACCEPT_ENCODING='' \
    HTTP_ACCEPT_ENCODING='gzip, deflate, br' \
    REQUEST_METHOD=GET \
    REQUEST_URI='/cgi/ThirdParty/fn-knock/index.cgi/' \
      sh "${cgi}"
  )"
  normalized_output="$(printf '%s' "${output}" | tr -d '\r')"
  if printf '%s' "${normalized_output}" | grep -Fq 'Content-Encoding:'; then
    fail "fnOS CGI forwarded a compressed representation: ${cgi}"
  fi
  printf '%s' "${normalized_output}" | grep -Fq 'Vary: Accept-Encoding' || \
    fail "raw response lost Vary: ${cgi}"
  printf '%s' "${normalized_output}" | grep -Fq \
    'Cache-Control: private, no-store, no-cache, max-age=0, must-revalidate' || \
    fail "index response is storable: ${cgi}"
  printf '%s' "${normalized_output}" | grep -Fq 'CDN-Cache-Control: no-store' || \
    fail "index response allows CDN storage: ${cgi}"
  if printf '%s' "${normalized_output}" | grep -Fq \
    'Cache-Control: public, max-age=31536000, immutable'; then
    fail "index response retained the asset cache policy: ${cgi}"
  fi
  printf '%s' "${normalized_output}" | grep -Fq 'raw-src="/fixture.js"' || \
    fail "raw response body was changed: ${cgi}"

  fallback_output="$(
    PATH="${WORK_DIR}/bin:${PATH}" \
    EXPECTED_ACCEPT_ENCODING='' \
    HTTP_ACCEPT_ENCODING='gzip, deflate, br' \
    REQUEST_METHOD=GET \
    REQUEST_URI='/cgi/ThirdParty/fn-knock/index.cgi/settings' \
      sh "${cgi}"
  )"
  printf '%s' "${fallback_output}" | tr -d '\r' | grep -Fq \
    'Cache-Control: private, no-store, no-cache, max-age=0, must-revalidate' || \
    fail "HTML SPA fallback is storable: ${cgi}"

  asset_output="$(
    PATH="${WORK_DIR}/bin:${PATH}" \
    EXPECTED_ACCEPT_ENCODING='' \
    HTTP_ACCEPT_ENCODING='gzip, deflate, br' \
    REQUEST_METHOD=GET \
    REQUEST_URI='/cgi/ThirdParty/fn-knock/index.cgi/assets/app-ABCDEFG.js' \
      sh "${cgi}"
  )"
  printf '%s' "${asset_output}" | tr -d '\r' | grep -Fq \
    'Cache-Control: public, max-age=31536000, immutable' || \
    fail "fingerprinted asset cache policy was not preserved: ${cgi}"
  printf '%s' "${asset_output}" | tr -d '\r' | grep -Fq \
    'X-Content-Type-Options: nosniff' || \
    fail "static asset lost nosniff protection: ${cgi}"
done

assert_fpk_terminal_forwarding() {
  local method="$1" request_uri="$2" query="$3" body="$4" expected_url="$5"
  local capture_file="${WORK_DIR}/terminal-forwarding.capture"
  printf '%s' "${body}" | \
    PATH="${WORK_DIR}/bin:${PATH}" \
    EXPECTED_ACCEPT_ENCODING='' \
    CAPTURE_FILE="${capture_file}" \
    ADMIN_TARGET_HOST='127.0.0.1' \
    ADMIN_TARGET_PORT='7998' \
    REQUEST_METHOD="${method}" \
    REQUEST_URI="${request_uri}" \
    QUERY_STRING="${query}" \
      sh "${FPK_CGI}" >/dev/null
  assert_contains "${capture_file}" "method=${method}" "FPK terminal method forwarding"
  assert_contains "${capture_file}" "url=${expected_url}" "FPK terminal query forwarding"
  assert_contains "${capture_file}" "body=${body}" "FPK terminal JSON body forwarding"
}

assert_fpk_terminal_forwarding \
  GET \
  '/cgi/ThirdParty/fn-knock/index.cgi/api/admin/terminal/local' \
  '' \
  '' \
  'http://127.0.0.1:7998/api/admin/terminal/local'
assert_fpk_terminal_forwarding \
  PATCH \
  '/cgi/ThirdParty/fn-knock/index.cgi/api/admin/terminal/local?force=true&confirmationToken=terminal-confirmation' \
  'force=true&confirmationToken=terminal-confirmation' \
  '{"enabled":false,"revision":1,"acknowledgeRisk":false}' \
  'http://127.0.0.1:7998/api/admin/terminal/local?force=true&confirmationToken=terminal-confirmation'
assert_fpk_terminal_forwarding \
  POST \
  '/cgi/ThirdParty/fn-knock/index.cgi/api/admin/terminal/local/sessions' \
  '' \
  '{"cols":120,"rows":32}' \
  'http://127.0.0.1:7998/api/admin/terminal/local/sessions'
assert_fpk_terminal_forwarding \
  GET \
  '/cgi/ThirdParty/fn-knock/index.cgi/api/admin/terminal/attachments/attachment-1/events?after=4&timeoutMs=25000' \
  'after=4&timeoutMs=25000' \
  '' \
  'http://127.0.0.1:7998/api/admin/terminal/attachments/attachment-1/events?after=4&timeoutMs=25000'

printf '[test-cgi-proxy-contract] CGI forwarding, local terminal, and fnOS WebView compression contract passed\n'
