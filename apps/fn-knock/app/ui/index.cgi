#!/bin/sh

RUNTIME_PORT_FILE=""

if [ -n "${TRIM_PKGVAR:-}" ]; then
    RUNTIME_PORT_FILE="${TRIM_PKGVAR}/runtime-ports.env"
elif [ -n "${cgiName:-}" ]; then
    RUNTIME_PORT_FILE="/var/apps/${cgiName}/var/runtime-ports.env"
elif [ -n "${SCRIPT_FILENAME:-}" ]; then
    case "${SCRIPT_FILENAME}" in
        */target/ui/index.cgi)
            RUNTIME_PORT_FILE="${SCRIPT_FILENAME%/target/ui/index.cgi}/var/runtime-ports.env"
            ;;
    esac
fi

if [ -n "${RUNTIME_PORT_FILE}" ] && [ -r "${RUNTIME_PORT_FILE}" ]; then
    . "${RUNTIME_PORT_FILE}"
fi

TARGET_HOST=${ADMIN_TARGET_HOST:-"127.0.0.1"}

if [ -n "$ADMIN_TARGET_PORT" ]; then
    TARGET_PORT="$ADMIN_TARGET_PORT"
elif [ -n "$BACKEND_PORT" ]; then
    TARGET_PORT="$BACKEND_PORT"
elif [ -n "$wizard_backend_port" ]; then
    TARGET_PORT="$wizard_backend_port"
elif [ -n "$ADMIN_VIEW_PORT" ]; then
    TARGET_PORT="$ADMIN_VIEW_PORT"
elif [ -n "$wizard_admin_view_port" ]; then
    TARGET_PORT="$wizard_admin_view_port"
else
    TARGET_PORT="7998"
fi

TARGET_SCHEME=${ADMIN_TARGET_SCHEME:-"http"}

guess_content_type() {
    case "$1" in
        *.js|*.mjs)       printf "Content-Type: text/javascript; charset=utf-8\r\n" ;;
        *.css)            printf "Content-Type: text/css; charset=utf-8\r\n" ;;
        *.html|"/"|*/ )   printf "Content-Type: text/html; charset=utf-8\r\n" ;;
        *.json|*.map|/api/*) printf "Content-Type: application/json; charset=utf-8\r\n" ;;
        *.svg)            printf "Content-Type: image/svg+xml\r\n" ;;
        *.png)            printf "Content-Type: image/png\r\n" ;;
        *.jpg|*.jpeg)     printf "Content-Type: image/jpeg\r\n" ;;
        *.gif)            printf "Content-Type: image/gif\r\n" ;;
        *.webp)           printf "Content-Type: image/webp\r\n" ;;
        *.ico)            printf "Content-Type: image/x-icon\r\n" ;;
        *.wasm)           printf "Content-Type: application/wasm\r\n" ;;
        *)                printf "Content-Type: application/octet-stream\r\n" ;;
    esac
}

emit_upstream_header() {
    HEADER_NAME="$1"
    HEADER_LINE=$(grep -i "^${HEADER_NAME}:" "$HEADER_FILE" | tail -1 | tr -d '\r')
    if [ -n "$HEADER_LINE" ]; then
        printf "%s\r\n" "$HEADER_LINE"
    fi
}

is_html_response() {
    case "${REL_PATH:-/}" in
        /|/index.html) return 0 ;;
    esac
    case "${CONTENT_TYPE_LINE:-}" in
        *[Tt][Ee][Xx][Tt]/[Hh][Tt][Mm][Ll]*) return 0 ;;
    esac
    return 1
}

emit_upstream_cache_headers() {
    if is_html_response; then
        # The FPK replaces fingerprinted assets during an upgrade. Never let
        # an HTML document outlive the asset generation it references,
        # including SPA fallbacks and authentication views.
        printf "Cache-Control: private, no-store, no-cache, max-age=0, must-revalidate\r\n"
        printf "CDN-Cache-Control: no-store\r\n"
        printf "Surrogate-Control: no-store\r\n"
        printf "Pragma: no-cache\r\n"
        printf "Expires: 0\r\n"
    else
        emit_upstream_header "Cache-Control"
        emit_upstream_header "CDN-Cache-Control"
        emit_upstream_header "Surrogate-Control"
        emit_upstream_header "Expires"
        emit_upstream_header "Pragma"
    fi
    emit_upstream_header "ETag"
    emit_upstream_header "Last-Modified"
    emit_upstream_header "Vary"
}

REQ_URI=${REQUEST_URI:-""}
URI_NO_QUERY="${REQ_URI%%\?*}"
QUERY_STRING=${QUERY_STRING:-""}

case "$URI_NO_QUERY" in
    */index.cgi)
        if [ -n "$QUERY_STRING" ]; then
            LOCATION="${URI_NO_QUERY}/?${QUERY_STRING}"
        else
            LOCATION="${URI_NO_QUERY}/"
        fi
        printf "Status: 302 Found\r\n"
        printf "Location: %s\r\n" "$LOCATION"
        printf "Content-Type: text/plain; charset=utf-8\r\n"
        printf "Cache-Control: private, no-store, no-cache, max-age=0, must-revalidate\r\n"
        printf "Pragma: no-cache\r\n"
        printf "Expires: 0\r\n\r\n"
        printf "Redirecting\n"
        exit 0
        ;;
esac

case "$URI_NO_QUERY" in
    *index.cgi*) REL_PATH="${URI_NO_QUERY#*index.cgi}" ;;
    *)           REL_PATH="$URI_NO_QUERY" ;;
esac

if [ -z "$REL_PATH" ]; then
    REL_PATH="/"
fi

case "$REL_PATH" in
    *..*)
        printf "Status: 400 Bad Request\r\n"
        printf "Content-Type: text/plain; charset=utf-8\r\n\r\n"
        printf "Bad Request\n"
        exit 1
        ;;
esac

TARGET_URL="${TARGET_SCHEME}://${TARGET_HOST}:${TARGET_PORT}${REL_PATH}"
if [ -n "$QUERY_STRING" ]; then
    TARGET_URL="${TARGET_URL}?${QUERY_STRING}"
fi

set -- -s

[ -n "$HTTP_X_TIMESTAMP" ]      && set -- "$@" -H "x-timestamp: $HTTP_X_TIMESTAMP"
[ -n "$HTTP_X_NONCE" ]          && set -- "$@" -H "x-nonce: $HTTP_X_NONCE"
[ -n "$HTTP_X_SIGNATURE" ]      && set -- "$@" -H "x-signature: $HTTP_X_SIGNATURE"
[ -n "$HTTP_X_REQUESTED_WITH" ] && set -- "$@" -H "x-requested-with: $HTTP_X_REQUESTED_WITH"
[ -n "$HTTP_ACCEPT" ]           && set -- "$@" -H "accept: $HTTP_ACCEPT"
# Do not negotiate a compressed representation across the fnOS CGI boundary.
# Some embedded Android/Huawei WebViews cache a Brotli module response with
# inconsistent representation metadata: the first load succeeds, then later
# loads fail before the module can execute. The loopback hop is local, so send
# raw bytes here and keep precompressed assets available to direct HTTP clients.
[ -n "$HTTP_ACCEPT_LANGUAGE" ]  && set -- "$@" -H "accept-language: $HTTP_ACCEPT_LANGUAGE"
[ -n "$HTTP_USER_AGENT" ]       && set -- "$@" -H "user-agent: $HTTP_USER_AGENT"
[ -n "$HTTP_ORIGIN" ]           && set -- "$@" -H "origin: $HTTP_ORIGIN"
[ -n "$HTTP_REFERER" ]          && set -- "$@" -H "referer: $HTTP_REFERER"


METHOD=${REQUEST_METHOD:-"GET"}
set -- "$@" -X "$METHOD"

case "$METHOD" in
    POST|PUT|PATCH|DELETE)
        REQ_CONTENT_TYPE=${CONTENT_TYPE:-"application/json"}
        set -- "$@" -H "Content-Type: $REQ_CONTENT_TYPE"
        set -- "$@" --data-binary @- 
        ;;
esac

HEADER_FILE=$(mktemp)
BODY_FILE=$(mktemp)

trap 'rm -f "$HEADER_FILE" "$BODY_FILE"' EXIT

curl "$@" -D "$HEADER_FILE" -o "$BODY_FILE" "$TARGET_URL" >/dev/null 2>&1
CURL_EXIT=$?

if [ $CURL_EXIT -ne 0 ]; then
    printf "Status: 502 Bad Gateway\r\n"
    printf "Content-Type: text/plain; charset=utf-8\r\n"
    printf "Cache-Control: no-store\r\n\r\n"
    printf "连接后端失败。可能是 fn-knock 程序未启动，请尝试重启该应用。\n"
    exit 0
fi

STATUS_LINE=$(grep '^HTTP/' "$HEADER_FILE" | tail -1 | tr -d '\r')
STATUS_CODE=$(echo "$STATUS_LINE" | awk '{print $2}')
STATUS_TEXT=$(echo "$STATUS_LINE" | awk '{$1=""; $2=""; sub("^[ \t]+", ""); print}')

if [ "$STATUS_CODE" != "200" ] && [ -n "$STATUS_CODE" ]; then
    printf "Status: %s %s\r\n" "$STATUS_CODE" "$STATUS_TEXT"
fi

CONTENT_TYPE_LINE=$(grep -i '^content-type:' "$HEADER_FILE" | tail -1 | tr -d '\r')

if [ -n "$CONTENT_TYPE_LINE" ]; then
    printf "%s\r\n" "$CONTENT_TYPE_LINE"
else
    guess_content_type "$REL_PATH"
fi

emit_upstream_cache_headers
emit_upstream_header "Content-Length"
emit_upstream_header "Content-Disposition"
emit_upstream_header "X-Content-Type-Options"
printf "\r\n"
cat "$BODY_FILE"
