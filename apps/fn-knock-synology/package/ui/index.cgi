#!/bin/sh

PACKAGE_NAME="fn-knock-synology"
RUNTIME_PORT_FILE="/var/packages/${PACKAGE_NAME}/var/runtime-ports.env"
AUTHENTICATE_CGI="/usr/syno/synoman/webman/modules/authenticate.cgi"

AUTHENTICATED_USER=""
if [ -x "${AUTHENTICATE_CGI}" ]; then
    AUTHENTICATED_USER="$("${AUTHENTICATE_CGI}" 2>/dev/null)"
fi

if [ -z "${AUTHENTICATED_USER}" ]; then
    printf 'Status: 403 Forbidden\r\n'
    printf 'Content-Type: text/plain; charset=utf-8\r\n\r\n'
    printf 'DSM authentication required.\n'
    exit 0
fi

DSM_USER_GROUPS="$(id -Gn "${AUTHENTICATED_USER}" 2>/dev/null)" || DSM_USER_GROUPS=""
case " ${DSM_USER_GROUPS} " in
    *" administrators "*) ;;
    *)
        printf 'Status: 403 Forbidden\r\n'
        printf 'Content-Type: text/plain; charset=utf-8\r\n\r\n'
        printf 'DSM administrator privileges required.\n'
        exit 0
        ;;
esac

if [ -r "${RUNTIME_PORT_FILE}" ]; then
    . "${RUNTIME_PORT_FILE}"
fi

TARGET_HOST="${ADMIN_TARGET_HOST:-127.0.0.1}"
TARGET_PORT="${ADMIN_TARGET_PORT:-${BACKEND_PORT:-7998}}"
REQ_URI="${REQUEST_URI:-}"
URI_NO_QUERY="${REQ_URI%%\?*}"
QUERY_STRING="${QUERY_STRING:-}"

case "${URI_NO_QUERY}" in
    */index.cgi)
        if [ -n "${QUERY_STRING}" ]; then
            LOCATION="${URI_NO_QUERY}/?${QUERY_STRING}"
        else
            LOCATION="${URI_NO_QUERY}/"
        fi
        printf 'Status: 302 Found\r\n'
        printf 'Location: %s\r\n' "${LOCATION}"
        printf 'Content-Type: text/plain; charset=utf-8\r\n\r\n'
        printf 'Redirecting\n'
        exit 0
        ;;
esac

case "${URI_NO_QUERY}" in
    *index.cgi*) REL_PATH="${URI_NO_QUERY#*index.cgi}" ;;
    *) REL_PATH="${URI_NO_QUERY}" ;;
esac

[ -n "${REL_PATH}" ] || REL_PATH="/"
case "${REL_PATH}" in
    *..*)
        printf 'Status: 400 Bad Request\r\n'
        printf 'Content-Type: text/plain; charset=utf-8\r\n\r\n'
        printf 'Bad Request\n'
        exit 0
        ;;
esac

TARGET_URL="http://${TARGET_HOST}:${TARGET_PORT}${REL_PATH}"
if [ -n "${QUERY_STRING}" ]; then
    TARGET_URL="${TARGET_URL}?${QUERY_STRING}"
fi

REQUEST_METHOD_ORIGINAL="${REQUEST_METHOD:-GET}"
UPSTREAM_METHOD="${REQUEST_METHOD_ORIGINAL}"
if [ "${REQUEST_METHOD_ORIGINAL}" = "POST" ]; then
    case "${HTTP_X_HTTP_METHOD_OVERRIDE:-}" in
        PUT|PATCH|DELETE) UPSTREAM_METHOD="${HTTP_X_HTTP_METHOD_OVERRIDE}" ;;
    esac
fi

HEADER_FILE="$(mktemp)" || exit 1
BODY_FILE="$(mktemp)" || {
    rm -f "${HEADER_FILE}"
    exit 1
}
REQUEST_BODY_FILE="$(mktemp)" || {
    rm -f "${HEADER_FILE}" "${BODY_FILE}"
    exit 1
}
trap 'rm -f "${HEADER_FILE}" "${BODY_FILE}" "${REQUEST_BODY_FILE}"' EXIT

set -- -sS -D "${HEADER_FILE}" -o "${BODY_FILE}" -X "${UPSTREAM_METHOD}"
[ -n "${HTTP_X_TIMESTAMP:-}" ] && set -- "$@" -H "x-timestamp: ${HTTP_X_TIMESTAMP}"
[ -n "${HTTP_X_NONCE:-}" ] && set -- "$@" -H "x-nonce: ${HTTP_X_NONCE}"
[ -n "${HTTP_X_SIGNATURE:-}" ] && set -- "$@" -H "x-signature: ${HTTP_X_SIGNATURE}"
[ -n "${HTTP_X_REQUESTED_WITH:-}" ] && set -- "$@" -H "x-requested-with: ${HTTP_X_REQUESTED_WITH}"
[ -n "${HTTP_ACCEPT:-}" ] && set -- "$@" -H "accept: ${HTTP_ACCEPT}"
[ -n "${HTTP_ACCEPT_LANGUAGE:-}" ] && set -- "$@" -H "accept-language: ${HTTP_ACCEPT_LANGUAGE}"
[ -n "${HTTP_USER_AGENT:-}" ] && set -- "$@" -H "user-agent: ${HTTP_USER_AGENT}"
[ -n "${HTTP_ORIGIN:-}" ] && set -- "$@" -H "origin: ${HTTP_ORIGIN}"
[ -n "${HTTP_REFERER:-}" ] && set -- "$@" -H "referer: ${HTTP_REFERER}"

case "${REQUEST_METHOD_ORIGINAL}" in
    POST|PUT|PATCH|DELETE)
        case "${CONTENT_LENGTH:-0}" in
            ''|*[!0-9]*)
                printf 'Status: 400 Bad Request\r\n'
                printf 'Content-Type: text/plain; charset=utf-8\r\n\r\n'
                printf 'Invalid Content-Length.\n'
                exit 0
                ;;
        esac
        if [ "${CONTENT_LENGTH:-0}" -gt 0 ]; then
            dd bs=1 count="${CONTENT_LENGTH}" of="${REQUEST_BODY_FILE}" 2>/dev/null
        fi
        set -- "$@" -H "Content-Type: ${CONTENT_TYPE:-application/json}" --data-binary "@${REQUEST_BODY_FILE}"
        ;;
esac

if ! curl "$@" "${TARGET_URL}"; then
    printf 'Status: 502 Bad Gateway\r\n'
    printf 'Content-Type: text/plain; charset=utf-8\r\n\r\n'
    printf 'fn-knock is not available. Start the package in Package Center.\n'
    exit 0
fi

STATUS_LINE="$(grep '^HTTP/' "${HEADER_FILE}" | tail -1 | tr -d '\r')"
STATUS_CODE="$(printf '%s\n' "${STATUS_LINE}" | awk '{print $2}')"
STATUS_TEXT="$(printf '%s\n' "${STATUS_LINE}" | awk '{$1=""; $2=""; sub("^[ \t]+", ""); print}')"
CONTENT_TYPE_LINE="$(grep -i '^content-type:' "${HEADER_FILE}" | tail -1 | tr -d '\r')"
CACHE_CONTROL_LINE="$(grep -i '^cache-control:' "${HEADER_FILE}" | tail -1 | tr -d '\r')"

if [ -n "${STATUS_CODE}" ] && [ "${STATUS_CODE}" != "200" ]; then
    printf 'Status: %s %s\r\n' "${STATUS_CODE}" "${STATUS_TEXT}"
fi
if [ -n "${CONTENT_TYPE_LINE}" ]; then
    printf '%s\r\n' "${CONTENT_TYPE_LINE}"
else
    printf 'Content-Type: application/octet-stream\r\n'
fi
if [ -n "${CACHE_CONTROL_LINE}" ]; then
    printf '%s\r\n' "${CACHE_CONTROL_LINE}"
else
    case "${REL_PATH}" in
        /assets/*) printf 'Cache-Control: public, max-age=31536000, immutable\r\n' ;;
        /|/index.html) printf 'Cache-Control: no-cache, no-store, must-revalidate\r\n' ;;
    esac
fi
for HEADER_NAME in Expires Pragma ETag Last-Modified Vary; do
    HEADER_LINE="$(grep -i "^${HEADER_NAME}:" "${HEADER_FILE}" | tail -1 | tr -d '\r')"
    [ -z "${HEADER_LINE}" ] || printf '%s\r\n' "${HEADER_LINE}"
done
printf '\r\n'

if [ "${REL_PATH}" = "/" ] || [ "${REL_PATH}" = "/index.html" ]; then
    sed -e 's|src="/|src="./|g' -e 's|href="/|href="./|g' "${BODY_FILE}"
else
    cat "${BODY_FILE}"
fi
