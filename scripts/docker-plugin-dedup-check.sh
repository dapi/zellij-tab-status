#!/usr/bin/env bash
#
# Verify that repeated `zellij pipe --plugin ...` calls reuse one plugin instance.
# Runs INSIDE Docker container with an empty load_plugins config.
#
set -euo pipefail

PLUGIN_WASM="/test/plugin.wasm"
PLUGIN_PATH="file:$PLUGIN_WASM"
SESSION="plugin-dedup-check"
LOG_FILE="/tmp/zellij-0/zellij-log/zellij.log"

if [[ ! -f "$PLUGIN_WASM" ]]; then
    echo "ERROR: Plugin not found at $PLUGIN_WASM"
    exit 1
fi

mkdir -p /root/.config/zellij /root/.cache/zellij

cat > /root/.config/zellij/config.kdl <<EOF
default_layout "compact"
EOF

cat > /root/.cache/zellij/permissions.kdl <<EOF
"$PLUGIN_WASM" {
    ReadApplicationState
    ChangeApplicationState
    ReadCliPipes
}
EOF

echo "Starting Zellij session '$SESSION' (ondemand mode)..."
export ZELLIJ_SESSION="$SESSION"
script -qfc "zellij --session $SESSION options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
ZELLIJ_PID=$!

cleanup() {
    zellij kill-session "$SESSION" 2>/dev/null || true
    wait $ZELLIJ_PID 2>/dev/null || true
}
trap cleanup EXIT

for i in $(seq 1 30); do
    if zellij list-sessions 2>/dev/null | grep -q "$SESSION"; then
        break
    fi
    if ! kill -0 $ZELLIJ_PID 2>/dev/null; then
        echo "ERROR: Zellij process died"
        exit 1
    fi
    sleep 0.5
done

if ! zellij list-sessions 2>/dev/null | grep -q "$SESSION"; then
    echo "ERROR: Zellij session did not start within 15s"
    exit 1
fi

sleep 2

payload='{"action":"get_version"}'

for _ in 1 2 3 4 5; do
    timeout 5s zellij pipe --plugin "$PLUGIN_PATH" -- "$payload" < /dev/null >/dev/null
    sleep 0.2
done

sleep 0.8

if [[ ! -f "$LOG_FILE" ]]; then
    echo "ERROR: Zellij log file not found at $LOG_FILE"
    exit 1
fi

load_count=$(grep -c "\\[tab-status\\] Plugin loaded" "$LOG_FILE" || true)

if [[ "$load_count" -ne 1 ]]; then
    echo "FAIL: Expected exactly 1 plugin load, got $load_count"
    grep "\\[tab-status\\] Plugin loaded" "$LOG_FILE" || true
    exit 1
fi

echo "PASS: Repeated --plugin calls reused a single plugin instance (load_count=1)"
