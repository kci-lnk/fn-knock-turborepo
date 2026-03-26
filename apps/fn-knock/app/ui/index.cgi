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
    # Values are written by cmd/main after port validation.
    . "${RUNTIME_PORT_FILE}"
fi

TARGET_HOST=${ADMIN_TARGET_HOST:-"127.0.0.1"}

if [ -n "$ADMIN_TARGET_PORT" ]; then
    TARGET_PORT="$ADMIN_TARGET_PORT"
elif [ -n "$BACKEND_PORT" ]; then
    TARGET_PORT="$BACKEND_PORT"
elif [ -n "$wizard_backend_port" ]; then
    TARGET_PORT="$wizard_backend_port"
else
    TARGET_PORT="7998"
fi

TARGET_SCHEME=${ADMIN_TARGET_SCHEME:-"http"}

guess_content_type() {
    case "$1" in
        *.js|*.mjs)
            printf "Content-Type: text/javascript; charset=utf-8\r\n"
            ;;
        *.css)
            printf "Content-Type: text/css; charset=utf-8\r\n"
            ;;
        *.html|"/"|*/ )
            printf "Content-Type: text/html; charset=utf-8\r\n"
            ;;
        *.json|*.map|/api/*)
            printf "Content-Type: application/json; charset=utf-8\r\n"
            ;;
        *.svg)
            printf "Content-Type: image/svg+xml\r\n"
            ;;
        *.png)
            printf "Content-Type: image/png\r\n"
            ;;
        *.jpg|*.jpeg)
            printf "Content-Type: image/jpeg\r\n"
            ;;
        *.gif)
            printf "Content-Type: image/gif\r\n"
            ;;
        *.webp)
            printf "Content-Type: image/webp\r\n"
            ;;
        *.ico)
            printf "Content-Type: image/x-icon\r\n"
            ;;
        *.wasm)
            printf "Content-Type: application/wasm\r\n"
            ;;
        *)
            printf "Content-Type: application/octet-stream\r\n"
            ;;
    esac
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
        printf "Content-Type: text/plain; charset=utf-8\r\n\r\n"
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
    ARCH=$(uname -m)
    case "$ARCH" in
        arm*|aarch64)
            if [ ! -x "/usr/bin/redis-server" ] && [ ! -x "/usr/local/bin/redis-server" ]; then
                printf "Status: 200 OK\r\n"
                printf "Content-Type: text/html; charset=utf-8\r\n\r\n"
                printf '<!DOCTYPE html>\n<html>\n<head>\n<meta charset="UTF-8">\n<title>正在配置环境</title>\n</head>\n<body>\n'
                
                printf '<div id="installing" style="text-align: center; margin-top: 50px; font-family: sans-serif;">\n'
                printf '  <h2 style="color: #f0ad4e;">正在自动安装缺少的运行环境（Redis），请稍候...</h2>\n'
                printf '  <p>大约需要 1 - 2 分钟完成下载和配置，请不要关闭或刷新本页面。</p>\n'
                printf '</div>\n'
                
                for i in $(seq 1 1024); do printf "<!-- padding buffer -->\n"; done
                
                export DEBIAN_FRONTEND=noninteractive
                apt-get update -qq >/dev/null 2>&1
                apt-get install -y redis-server >/dev/null 2>&1
                
                if [ -f /etc/redis/redis.conf ]; then
                    sed -i 's/^protected-mode yes/protected-mode no/g' /etc/redis/redis.conf
                    if ! grep -q "^protected-mode no" /etc/redis/redis.conf; then
                        echo "protected-mode no" >> /etc/redis/redis.conf
                    fi
                else
                    mkdir -p /etc/redis
                    echo "protected-mode no" > /etc/redis/redis.conf
                fi
                
                systemctl restart redis-server >/dev/null 2>&1
                systemctl enable redis-server >/dev/null 2>&1
                
                printf '<script>document.getElementById("installing").style.display = "none";</script>\n'
                printf '<div style="text-align: center; margin-top: 100px; font-family: sans-serif;">\n'
                printf '  <h1 style="color: #5cb85c; font-size: 56px; font-weight: bold;">环境安装及配置完成！</h1>\n'
                printf '  <h2 style="color: #333; margin-top: 30px;">请重新启用一下 fn-knock 敲门程序即可正常使用。</h2>\n'
                printf '</div>\n'
                printf '</body>\n</html>\n'
                exit 0
            fi
            ;;
    esac

    printf "Status: 502 Bad Gateway\r\n"
    printf "Content-Type: text/plain; charset=utf-8\r\n\r\n"
    printf "连接出错，请尝试重启敲门fknock程序\n"
    exit 0
fi

if [ "$REL_PATH" = "/" ] || [ "$REL_PATH" = "/index.html" ]; then
    printf "Content-Type: text/html; charset=utf-8\r\n\r\n"
    sed -e 's|src="/|src="./|g' -e 's|href="/|href="./|g' "$BODY_FILE"
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

printf "\r\n"

cat "$BODY_FILE"
