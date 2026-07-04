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
NODE_BIN_V20="/var/apps/nodejs_v20/target/bin/node"
NODE_BIN_V24="/var/apps/nodejs_v24/target/bin/node"

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

has_node_runtime() {
    if [ -x "$NODE_BIN_V20" ] || [ -x "$NODE_BIN_V24" ]; then
        return 0
    fi
    if command -v node >/dev/null 2>&1; then
        return 0
    fi
    return 1
}

check_redis_running() {
    if command -v lsof >/dev/null 2>&1; then
        lsof -i :6379 >/dev/null 2>&1 && return 0
    fi
    if command -v netstat >/dev/null 2>&1; then
        netstat -tln | grep :6379 >/dev/null 2>&1 && return 0
    fi
    return 1
}

render_missing_node_page() {
    printf "Status: 200 OK\r\n"
    printf "Content-Type: text/html; charset=utf-8\r\n\r\n"
    printf '<!DOCTYPE html>\n<html>\n<head>\n<meta charset="UTF-8">\n<title>缺少 Node.js 运行环境</title>\n</head>\n<body>\n'
    printf '<div style="max-width: 760px; margin: 80px auto; padding: 0 24px; font-family: sans-serif; text-align: center;">\n'
    printf '  <h1 style="color: #d9534f; font-size: 42px; font-weight: bold; margin-bottom: 24px;">当前无法启动 fn-knock</h1>\n'
    printf '  <p style="color: #333; font-size: 20px; line-height: 1.8; margin: 0 0 16px;">检测到当前设备为 ARM 架构，且系统中不存在可用的 Node.js 运行环境。</p>\n'
    printf '  <p style="color: #333; font-size: 20px; line-height: 1.8; margin: 0 0 16px;">请先前往应用商店安装 <strong>nodejs_v24</strong> 版本，安装完成后重新启用 fn-knock。</p>\n'
    printf '  <p style="color: #666; font-size: 16px; line-height: 1.8; margin: 0;">如果已经安装，请确认应用已成功启用后再刷新当前页面。</p>\n'
    printf '</div>\n'
    printf '</body>\n</html>\n'
}

render_missing_redis_page() {
    printf "Status: 200 OK\r\n"
    printf "Content-Type: text/html; charset=utf-8\r\n\r\n"

    printf '<!DOCTYPE html>\n'
    printf '<html lang="zh-CN">\n'
    printf '<head>\n'
    printf '  <meta charset="UTF-8">\n'
    printf '  <meta name="viewport" content="width=device-width, initial-scale=1.0">\n'
    printf '  <title>Redis 未就绪</title>\n'
    printf '  <style>\n'
    printf '    * { box-sizing: border-box; }\n'
    printf '    html, body {\n'
    printf '      margin: 0;\n'
    printf '      padding: 0;\n'
    printf '      background: #ffffff;\n'
    printf '      color: #111111;\n'
    printf '      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;\n'
    printf '    }\n'
    printf '    body {\n'
    printf '      min-height: 100vh;\n'
    printf '      display: flex;\n'
    printf '      align-items: center;\n'
    printf '      justify-content: center;\n'
    printf '      padding: 32px;\n'
    printf '    }\n'
    printf '    .wrap {\n'
    printf '      width: 100%%;\n'
    printf '      max-width: 760px;\n'
    printf '    }\n'
    printf '    .card {\n'
    printf '      border: 1px solid #e5e5e5;\n'
    printf '      padding: 40px;\n'
    printf '      background: #fff;\n'
    printf '    }\n'
    printf '    h1 {\n'
    printf '      margin: 0 0 16px;\n'
    printf '      font-size: 32px;\n'
    printf '      line-height: 1.2;\n'
    printf '      font-weight: 600;\n'
    printf '      color: #000;\n'
    printf '    }\n'
    printf '    p {\n'
    printf '      margin: 0 0 14px;\n'
    printf '      font-size: 16px;\n'
    printf '      line-height: 1.8;\n'
    printf '      color: #222;\n'
    printf '    }\n'
    printf '    .muted {\n'
    printf '      color: #666;\n'
    printf '    }\n'
    printf '    .section {\n'
    printf '      margin-top: 32px;\n'
    printf '      padding-top: 24px;\n'
    printf '      border-top: 1px solid #f0f0f0;\n'
    printf '    }\n'
    printf '    pre {\n'
    printf '      margin: 16px 0 0;\n'
    printf '      padding: 20px;\n'
    printf '      background: #0f0f0f;\n'
    printf '      color: #f5f5f5;\n'
    printf '      border: 1px solid #1f1f1f;\n'
    printf '      overflow-x: auto;\n'
    printf '      font-size: 13px;\n'
    printf '      line-height: 1.7;\n'
    printf '      white-space: pre;\n'
    printf '      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;\n'
    printf '    }\n'
    printf '    .hint {\n'
    printf '      margin-top: 20px;\n'
    printf '      padding: 16px 0 0;\n'
    printf '      border-top: 1px solid #f0f0f0;\n'
    printf '      font-size: 15px;\n'
    printf '      color: #111;\n'
    printf '    }\n'
    printf '    .btn {\n'
    printf '      display: inline-block;\n'
    printf '      margin-top: 28px;\n'
    printf '      padding: 12px 20px;\n'
    printf '      background: #111;\n'
    printf '      color: #fff;\n'
    printf '      border: 1px solid #111;\n'
    printf '      text-decoration: none;\n'
    printf '      font-size: 14px;\n'
    printf '      cursor: pointer;\n'
    printf '      transition: all 0.2s ease;\n'
    printf '    }\n'
    printf '    .btn:hover {\n'
    printf '      background: #fff;\n'
    printf '      color: #111;\n'
    printf '    }\n'
    printf '    code {\n'
    printf '      background: #f5f5f5;\n'
    printf '      padding: 2px 6px;\n'
    printf '      border: 1px solid #ececec;\n'
    printf '      font-size: 13px;\n'
    printf '    }\n'
    printf '    @media (max-width: 640px) {\n'
    printf '      .card { padding: 24px; }\n'
    printf '      h1 { font-size: 26px; }\n'
    printf '      p { font-size: 15px; }\n'
    printf '    }\n'
    printf '  </style>\n'
    printf '</head>\n'
    printf '<body>\n'
    printf '  <div class="wrap">\n'
    printf '    <div class="card">\n'
    printf '      <h1>Redis 服务未就绪</h1>\n'
    printf '      <p>检测到当前环境缺少可用的 Redis 实例，<strong>fn-knock</strong> 需要连接到 <code>127.0.0.1:6379</code>，且 Redis 需保持<strong>无密码</strong>状态，应用才可正常运行。</p>\n'
    printf '      <p class="muted">建议直接使用以下 Docker Compose 配置进行部署。</p>\n'
    printf '\n'
    printf '      <div class="section">\n'
    printf '        <pre>version: "3.8"\n'
    printf '\n'
    printf 'services:\n'
    printf '  redis:\n'
    printf '    image: redis:7-alpine\n'
    printf '    container_name: redis\n'
    printf '    restart: unless-stopped\n'
    printf '    ports:\n'
    printf '      - "127.0.0.1:6379:6379"\n'
    printf '    volumes:\n'
    printf '      - ./data:/data\n'
    printf '    command: >\n'
    printf '      redis-server\n'
    printf '      --bind 0.0.0.0\n'
    printf '      --protected-mode no\n'
    printf '      --appendonly yes\n'
    printf '      --save 900 1\n'
    printf '      --save 300 10\n'
    printf '      --save 60 10000</pre>\n'
    printf '      </div>\n'
    printf '\n'
    printf '      <div class="hint">\n'
    printf '        Redis 部署完成后，请前往<strong>应用管理</strong> → <strong>已安装应用</strong>，重新启用 <strong>敲门knock</strong>。\n'
    printf '      </div>\n'
    printf '\n'
    printf '    </div>\n'
    printf '  </div>\n'
    printf '</body>\n'
    printf '</html>\n'
}

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

emit_upstream_cache_headers() {
    emit_upstream_header "Cache-Control"
    emit_upstream_header "CDN-Cache-Control"
    emit_upstream_header "Surrogate-Control"
    emit_upstream_header "Expires"
    emit_upstream_header "Pragma"
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
            if ! has_node_runtime; then
                render_missing_node_page
                exit 0
            fi
            ;;
    esac

    if ! check_redis_running; then
        render_missing_redis_page
        exit 0
    fi

    printf "Status: 502 Bad Gateway\r\n"
    printf "Content-Type: text/plain; charset=utf-8\r\n\r\n"
    printf "连接后端失败。可能是 fn-knock 程序未启动，请尝试重启该应用。\n"
    exit 0
fi

if [ "$REL_PATH" = "/" ] || [ "$REL_PATH" = "/index.html" ]; then
    printf "Content-Type: text/html; charset=utf-8\r\n"
    emit_upstream_cache_headers
    printf "\r\n"
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

emit_upstream_cache_headers
printf "\r\n"
cat "$BODY_FILE"
