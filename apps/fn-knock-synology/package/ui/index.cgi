#!/bin/sh

PACKAGE_NAME="fn-knock-synology"
RUNTIME_PORT_FILE="/var/packages/${PACKAGE_NAME}/var/runtime-ports.env"
AUTHENTICATE_CGI="${AUTHENTICATE_CGI:-/usr/syno/synoman/webman/modules/authenticate.cgi}"
SESSION_COOKIE_NAME="fn_knock_synotoken"
STAGED_COOKIE_NAME="fn_knock_synotoken_stage"
SESSION_COOKIE_PATH="/webman/3rdparty/${PACKAGE_NAME}/"
LAUNCHER_PATH="${SESSION_COOKIE_PATH}launch.html"
AUTH_BOOTSTRAP_QUERY="fn_knock_auth_bootstrap=1"
ORIGINAL_QUERY_STRING="${QUERY_STRING:-}"
REQUEST_METHOD_ORIGINAL="${REQUEST_METHOD:-GET}"
REQ_URI="${REQUEST_URI:-}"
URI_NO_QUERY="${REQ_URI%%\?*}"

COOKIE_SECURE_ATTR=""
case "${HTTPS:-}:${HTTP_X_FORWARDED_PROTO:-}" in
    on:*|1:*|*:https) COOKIE_SECURE_ATTR="; Secure" ;;
esac

cookie_value() {
    printf '%s' "${HTTP_COOKIE:-}" |
        tr ';' '\n' |
        sed -n "s/^[[:space:]]*$1=//p" |
        head -n 1
}

SESSION_TOKEN_ENCODED="$(cookie_value "${SESSION_COOKIE_NAME}")"
case "${SESSION_TOKEN_ENCODED}" in
    ''|*[!A-Za-z0-9._~%+-]*) SESSION_TOKEN_ENCODED="" ;;
esac

STAGED_TOKEN_ENCODED="$(cookie_value "${STAGED_COOKIE_NAME}")"
case "${STAGED_TOKEN_ENCODED}" in
    ''|*[!A-Za-z0-9._~%+-]*) STAGED_TOKEN_ENCODED="" ;;
esac

AUTH_TOKEN_ENCODED="${STAGED_TOKEN_ENCODED:-${SESSION_TOKEN_ENCODED}}"

emit_session_cookies() {
    [ -z "${AUTH_TOKEN_ENCODED}" ] || \
        printf 'Set-Cookie: %s=%s; Path=%s%s; HttpOnly; SameSite=Strict\r\n' \
            "${SESSION_COOKIE_NAME}" "${AUTH_TOKEN_ENCODED}" \
            "${SESSION_COOKIE_PATH}" "${COOKIE_SECURE_ATTR}"
    [ -z "${STAGED_TOKEN_ENCODED}" ] || \
        printf 'Set-Cookie: %s=; Path=%s%s; HttpOnly; SameSite=Strict; Max-Age=0\r\n' \
            "${STAGED_COOKIE_NAME}" "${SESSION_COOKIE_PATH}" "${COOKIE_SECURE_ATTR}"
}

clear_session_cookies() {
    for COOKIE_NAME in "${SESSION_COOKIE_NAME}" "${STAGED_COOKIE_NAME}"; do
        printf 'Set-Cookie: %s=; Path=%s%s; HttpOnly; SameSite=Strict; Max-Age=0\r\n' \
            "${COOKIE_NAME}" "${SESSION_COOKIE_PATH}" "${COOKIE_SECURE_ATTR}"
    done
}

AUTH_QUERY_STRING="${ORIGINAL_QUERY_STRING}"
if [ -n "${AUTH_TOKEN_ENCODED}" ]; then
    case "&${AUTH_QUERY_STRING}&" in
        *'&SynoToken='*) ;;
        *)
            if [ -n "${AUTH_QUERY_STRING}" ]; then
                AUTH_QUERY_STRING="${AUTH_QUERY_STRING}&SynoToken=${AUTH_TOKEN_ENCODED}"
            else
                AUTH_QUERY_STRING="SynoToken=${AUTH_TOKEN_ENCODED}"
            fi
            ;;
    esac
fi

AUTHENTICATED_USER=""
if [ -x "${AUTHENTICATE_CGI}" ]; then
    AUTHENTICATED_USER="$(QUERY_STRING="${AUTH_QUERY_STRING}" "${AUTHENTICATE_CGI}" 2>/dev/null)"
fi

if [ -z "${AUTHENTICATED_USER}" ]; then
    case "${REQUEST_METHOD_ORIGINAL}:${URI_NO_QUERY}:${ORIGINAL_QUERY_STRING}" in
        GET:*/index.cgi:|GET:*/index.cgi/:)
            printf 'Status: 302 Found\r\n'
            printf 'Location: %s\r\n' "${LAUNCHER_PATH}"
            clear_session_cookies
            printf 'Cache-Control: no-cache, no-store, must-revalidate\r\n'
            printf 'Content-Type: text/plain; charset=utf-8\r\n\r\n'
            printf 'Redirecting to DSM session launcher.\n'
            ;;
        *)
            printf 'Status: 403 Forbidden\r\n'
            clear_session_cookies
            printf 'Content-Type: text/plain; charset=utf-8\r\n\r\n'
            printf 'DSM authentication required.\n'
            ;;
    esac
    exit 0
fi

DSM_USER_GROUPS="$(id -Gn "${AUTHENTICATED_USER}" 2>/dev/null)" || DSM_USER_GROUPS=""
case " ${DSM_USER_GROUPS} " in
    *" administrators "*) ;;
    *)
        printf 'Status: 403 Forbidden\r\n'
        emit_session_cookies
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
QUERY_STRING="${ORIGINAL_QUERY_STRING}"
if [ "${QUERY_STRING}" = "${AUTH_BOOTSTRAP_QUERY}" ]; then
    QUERY_STRING=""
fi

case "${URI_NO_QUERY}" in
    */index.cgi)
        if [ -n "${QUERY_STRING}" ]; then
            LOCATION="${URI_NO_QUERY}/?${QUERY_STRING}"
        else
            LOCATION="${URI_NO_QUERY}/"
        fi
        printf 'Status: 302 Found\r\n'
        printf 'Location: %s\r\n' "${LOCATION}"
        emit_session_cookies
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
        emit_session_cookies
        printf 'Content-Type: text/plain; charset=utf-8\r\n\r\n'
        printf 'Bad Request\n'
        exit 0
        ;;
esac

TARGET_URL="http://${TARGET_HOST}:${TARGET_PORT}${REL_PATH}"
if [ -n "${QUERY_STRING}" ]; then
    TARGET_URL="${TARGET_URL}?${QUERY_STRING}"
fi

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
                emit_session_cookies
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
    emit_session_cookies
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
emit_session_cookies
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
