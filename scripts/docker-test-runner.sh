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

# --- Start Zellij headlessly ---
# `script` provides the pseudo-TTY that Zellij requires.
echo "Starting Zellij session '$SESSION'..."
script -qfc "zellij --session $SESSION options --disable-mouse-mode" /dev/null &
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

# --- Discover pane ID ---
export ZELLIJ_SESSION="$SESSION"

# Let Zellij fully initialize and deliver TabUpdate/PaneUpdate
sleep 2

PANE_ID=$(zellij action dump-layout 2>/dev/null \
    | grep -oP 'terminal_pane_id="\K[0-9]+' \
    | head -1)

if [[ -z "$PANE_ID" ]]; then
    # Fallback: try numeric IDs from dump-layout
    PANE_ID=$(zellij action dump-layout 2>/dev/null \
        | grep -oP 'pane_id="\K[0-9]+' \
        | head -1)
fi

if [[ -z "$PANE_ID" ]]; then
    echo "ERROR: Could not discover pane ID from dump-layout"
    echo "Layout dump:"
    zellij action dump-layout 2>/dev/null || echo "(dump failed)"
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
