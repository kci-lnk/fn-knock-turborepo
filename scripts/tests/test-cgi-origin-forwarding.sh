#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTER="${ROOT_DIR}/apps/server-admin-rs/src/app/router.rs"
ADMIN_CLIENT="${ROOT_DIR}/apps/server-admin-view/src/lib/api/client.ts"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-cgi-compression.XXXXXX")"
trap 'rm -rf "${WORK_DIR}"' EXIT

fail() {
  printf '[test-cgi-origin-forwarding] ERROR: %s\n' "$*" >&2
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
  assert_contains "${cgi}" 'HTTP_HOST' 'external CGI authority forwarding'
  assert_contains "${cgi}" '-H "host:' 'loopback request Host override'
  assert_contains "${cgi}" 'HTTP_ORIGIN' 'browser Origin forwarding'
  assert_contains "${cgi}" 'HTTP_SEC_FETCH_SITE' 'Fetch Metadata forwarding'
  assert_contains "${cgi}" '-H "sec-fetch-site:' 'Fetch Metadata header'
  assert_contains "${cgi}" 'HTTP_X_FN_KNOCK_BROWSER_ORIGIN' 'CGI browser-origin proof forwarding'
  assert_contains "${cgi}" '-H "x-fn-knock-browser-origin:' 'CGI browser-origin proof header'
  assert_contains "${cgi}" 'PUBLIC_SCHEME' 'external CGI scheme resolution'
  assert_contains "${cgi}" '-H "x-forwarded-proto:' 'external CGI scheme forwarding'
  assert_contains "${cgi}" 'HTTP_ACCEPT_ENCODING' 'compression negotiation forwarding'
  assert_contains "${cgi}" '-H "accept-encoding:' 'Accept-Encoding upstream header'
  assert_contains "${cgi}" 'Content-Encoding' 'compressed response encoding forwarding'
  assert_contains "${cgi}" 'Content-Length' 'compressed response length forwarding'
  assert_not_contains "${cgi}" 's|src="/|src="./|g' 'compressed response body mutation'
done

mkdir -p "${WORK_DIR}/bin"
cat > "${WORK_DIR}/bin/curl" <<'FAKE_CURL'
#!/bin/sh
header_file=""
body_file=""
accept_encoding=""
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
      shift
      ;;
  esac
done
[ "${accept_encoding}" = "${EXPECTED_ACCEPT_ENCODING}" ] || exit 65
body='compressed-src="/fixture.js"'
printf 'HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Encoding: br\r\nContent-Length: %s\r\nVary: Accept-Encoding\r\n\r\n' \
  "${#body}" > "${header_file}"
printf '%s' "${body}" > "${body_file}"
FAKE_CURL
chmod 755 "${WORK_DIR}/bin/curl"

for cgi in \
  "${ROOT_DIR}/apps/fn-knock/app/ui/index.cgi" \
  "${ROOT_DIR}/apps/fn-knock-lite/app/ui/index.cgi"
do
  output="$(
    PATH="${WORK_DIR}/bin:${PATH}" \
    EXPECTED_ACCEPT_ENCODING='gzip, deflate, br' \
    HTTP_ACCEPT_ENCODING='gzip, deflate, br' \
    REQUEST_METHOD=GET \
    REQUEST_URI='/cgi/ThirdParty/fn-knock/index.cgi/' \
      sh "${cgi}"
  )"
  normalized_output="$(printf '%s' "${output}" | tr -d '\r')"
  printf '%s' "${normalized_output}" | grep -Fq 'Content-Encoding: br' || \
    fail "compressed response lost Content-Encoding: ${cgi}"
  printf '%s' "${normalized_output}" | grep -Fq 'Vary: Accept-Encoding' || \
    fail "compressed response lost Vary: ${cgi}"
  printf '%s' "${normalized_output}" | grep -Fq 'compressed-src="/fixture.js"' || \
    fail "compressed response body was changed: ${cgi}"
done

assert_contains "${ROUTER}" 'eq_ignore_ascii_case("cross-site")' 'cross-site mutation rejection'
assert_contains "${ROUTER}" 'origin.port_or_known_default() == expected.port_or_known_default()' 'origin authority comparison'
assert_contains "${ROUTER}" 'x-fn-knock-browser-origin' 'loopback CGI browser-origin proof validation'
assert_contains "${ADMIN_CLIENT}" 'X-Fn-Knock-Browser-Origin' 'same-origin frontend proof header'

printf '[test-cgi-origin-forwarding] CGI forwarding and compression contract passed\n'
