#!/usr/bin/env bash
#
# Docker test runner for zellij-tab-status integration tests.
# Runs INSIDE the Docker container. Starts Zellij headlessly,
# discovers pane ID, runs integration tests, cleans up.
#
set -euo pipefail

PLUGIN_WASM="/test/plugin.wasm"
SESSION="integration-test"

# --- Verify plugin exists ---
if [[ ! -f "$PLUGIN_WASM" ]]; then
    echo "ERROR: Plugin not found at $PLUGIN_WASM"
    echo "Mount it with: -v path/to/plugin.wasm:/test/plugin.wasm:ro"
    exit 1
fi

# --- Configure Zellij ---
# Pre-load plugin via config (avoids --plugin flag which creates duplicate instances)
# Pre-approve permissions (no UI to approve in headless mode)
mkdir -p /root/.config/zellij /root/.cache/zellij

cat > /root/.config/zellij/config.kdl <<EOF
default_layout "compact"
load_plugins {
    "file:$PLUGIN_WASM"
}
EOF

cat > /root/.cache/zellij/permissions.kdl <<EOF
"$PLUGIN_WASM" {
    ReadApplicationState
    ChangeApplicationState
    ReadCliPipes
}
EOF

# --- Start Zellij headlessly ---
# `script` provides the pseudo-TTY that Zellij requires.
echo "Starting Zellij session '$SESSION'..."
export ZELLIJ_SESSION="$SESSION"
script -qfc "zellij --session $SESSION options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
ZELLIJ_PID=$!

# Wait for session to be ready
for i in $(seq 1 30); do
    if zellij list-sessions 2>/dev/null | grep -q "$SESSION"; then
        echo "Zellij session ready (attempt $i)"
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
    kill $ZELLIJ_PID 2>/dev/null || true
    exit 1
fi

# Wait for plugin WASM compilation and initialization
sleep 5

# Close floating "about" pane to focus terminal
zellij action toggle-floating-panes
sleep 1

# --- Discover pane ID ---
# Write command into the terminal pane to export ZELLIJ_PANE_ID
zellij action write-chars 'echo $ZELLIJ_PANE_ID > /tmp/pane_id'
zellij action write 13
sleep 2

PANE_ID=$(cat /tmp/pane_id 2>/dev/null | tr -d '[:space:]')

if [[ -z "$PANE_ID" ]]; then
    echo "ERROR: Could not discover pane ID"
    kill $ZELLIJ_PID 2>/dev/null || true
    exit 1
fi

echo "Discovered pane ID: $PANE_ID"

# --- Run integration tests ---
export ZELLIJ_PANE_ID="$PANE_ID"
export PLUGIN_PATH="file:$PLUGIN_WASM"

test_exit=0
/test/scripts/integration-test.sh || test_exit=$?

# --- Cleanup ---
zellij kill-session "$SESSION" 2>/dev/null || true
wait $ZELLIJ_PID 2>/dev/null || true

exit $test_exit
