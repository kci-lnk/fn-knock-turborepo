#!/bin/bash
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for package in fn-knock fn-knock-lite; do
    (
        export TRIM_PKGVAR="$work" TRIM_TEMP_LOGFILE="$work/message"
        # Load the real functions without invoking the App Center command dispatcher.
        sed '/^# Serialize app-center invocations/,$d' "$root/apps/$package/cmd/main" > "$work/functions.sh"
        source "$work/functions.sh"
        unset FN_KNOCK_CLOUDFLARE_INGRESS_PORT wizard_cloudflare_ingress_port
        load_runtime_ports
        default=17999
        if [[ "$package" == fn-knock-lite ]]; then default=18999; fi
        [[ "$FN_KNOCK_CLOUDFLARE_INGRESS_PORT" == "$default" ]]
        validate_runtime_ports
        unset FN_KNOCK_CLOUDFLARE_INGRESS_PORT
        wizard_cloudflare_ingress_port=27999
        load_runtime_ports
        validate_runtime_ports
        [[ "$(printenv FN_KNOCK_CLOUDFLARE_INGRESS_PORT)" == 27999 ]]
        for key in ADMIN_VIEW_PORT BACKEND_PORT AUTH_PORT GO_BACKEND_PORT GO_REPROXY_PORT; do
            FN_KNOCK_CLOUDFLARE_INGRESS_PORT="${!key}"
            if validate_runtime_ports; then exit 1; fi
            grep -Fq "$key" "$TRIM_TEMP_LOGFILE"
            grep -Fq FN_KNOCK_CLOUDFLARE_INGRESS_PORT "$TRIM_TEMP_LOGFILE"
        done
        for invalid in 0 65536 word; do
            FN_KNOCK_CLOUDFLARE_INGRESS_PORT="$invalid"
            if validate_runtime_ports; then exit 1; fi
        done
        # Exercise the App Center callback with running PID files. Invalid changes
        # must return before its status/stop/start sequence touches either service.
        sleep 60 &
        live=$!
        trap 'kill "$live" 2>/dev/null || true' EXIT
        echo "$live" > "$GATEWAY_PID_FILE"
        echo "$live" > "$BACKEND_PID_FILE"
        FN_KNOCK_CLOUDFLARE_INGRESS_PORT="$BACKEND_PORT"
        if bash "$root/apps/$package/cmd/config_callback"; then exit 1; fi
        kill -0 "$live"
        grep -Fq BACKEND_PORT "$TRIM_TEMP_LOGFILE"
        grep -Fq FN_KNOCK_CLOUDFLARE_INGRESS_PORT "$TRIM_TEMP_LOGFILE"
        FN_KNOCK_CLOUDFLARE_INGRESS_PORT=27999
        echo "$$" > "$BACKEND_PID_FILE"
        (exit 98) &
        echo "$!" > "$GATEWAY_PID_FILE"
        if wait_runtime_ready; then exit 1; fi
        [[ "$START_FAILURE" == *"127.0.0.1:27999 已被占用"* ]]
        START_FAILURE=""
        (exit 1) &
        echo "$!" > "$GATEWAY_PID_FILE"
        if wait_runtime_ready; then exit 1; fi
        [[ "$START_FAILURE" == "Gateway exited"* ]]
        echo "$$" > "$GATEWAY_PID_FILE"
        START_TIMEOUT_SECONDS=1
        if wait_runtime_ready; then exit 1; fi
        [[ "$START_FAILURE" == *"timed out after 1 seconds." ]]
    )
done
# Remote deployment must reject duplicate or Lite-privileged ports before SSH.
mkdir "$work/bin"
cat > "$work/bin/ssh" <<'EOF'
#!/bin/sh
touch "$SSH_SENTINEL"
exit 1
EOF
chmod +x "$work/bin/ssh"
for package in fn-knock fn-knock-lite; do
    port=7998
    if [[ "$package" == fn-knock-lite ]]; then port=80; fi
    if PATH="$work/bin:$PATH" SSH_SENTINEL="$work/ssh-called" \
       FN_KNOCK_LITE_GRPC_SYNC_GO_COMPLETED=1 FN_KNOCK_WIZARD_CLOUDFLARE_INGRESS_PORT="$port" \
       bash "$root/scripts/$package-deploy.sh" install-remote > "$work/deploy-log" 2>&1; then exit 1; fi
    [[ ! -e "$work/ssh-called" ]]
    grep -Fq FN_KNOCK_CLOUDFLARE_INGRESS_PORT "$work/deploy-log"
done

if env -u ADMIN_VIEW_PORT FN_KNOCK_CLOUDFLARE_INGRESS_PORT=7991 \
    bash "$root/deploy/docker/entrypoint.sh" > "$work/docker-log" 2>&1; then exit 1; fi
grep -Fq 'ADMIN_VIEW_PORT both use port 7991' "$work/docker-log"

printf '[test-fpk-cloudflare-ingress] defaults, overrides, collisions and readiness errors passed\n'
