#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTER="${ROOT_DIR}/apps/server-admin-rs/src/app/router.rs"
ADMIN_CLIENT="${ROOT_DIR}/apps/server-admin-view/src/lib/api/client.ts"
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

mkdir -p "${WORK_DIR}/bin"
cat > "${WORK_DIR}/bin/curl" <<'FAKE_CURL'
#!/bin/sh
header_file=""
body_file=""
accept_encoding=""
target_url=""
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
      shift 2
      ;;
    *)
      target_url="$1"
      shift
      ;;
  esac
done
[ "${accept_encoding}" = "${EXPECTED_ACCEPT_ENCODING}" ] || exit 65
body='raw-src="/fixture.js"'
case "${target_url}" in
  */assets/*) content_type='text/javascript; charset=utf-8' ;;
  *) content_type='text/html; charset=utf-8' ;;
esac
printf 'HTTP/1.1 200 OK\r\nContent-Type: %s\r\nCache-Control: public, max-age=31536000, immutable\r\nContent-Length: %s\r\nVary: Accept-Encoding\r\nX-Content-Type-Options: nosniff\r\n\r\n' \
  "${content_type}" "${#body}" > "${header_file}"
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

printf '[test-cgi-proxy-contract] CGI forwarding and fnOS WebView compression contract passed\n'
