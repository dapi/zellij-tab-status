#!/usr/bin/env bash
#
# Docker test runner for zellij-tab-status CLI integration tests.
# Runs INSIDE the Docker container. Starts Zellij headlessly,
# discovers pane ID, runs integration tests, cleans up.
#
set -euo pipefail

CLI_BINARY="/test/zellij-tab-status"
SESSION="integration-test"

# Verify CLI binary exists
if [[ ! -f "$CLI_BINARY" ]]; then
    echo "ERROR: CLI binary not found at $CLI_BINARY"
    exit 1
fi
# Put binary on PATH
cp "$CLI_BINARY" /usr/local/bin/zellij-tab-status
chmod +x /usr/local/bin/zellij-tab-status

# Configure Zellij
mkdir -p /root/.config/zellij
cat > /root/.config/zellij/config.kdl <<EOF
default_layout "compact"
EOF

# Start Zellij headlessly (script provides PTY)
echo "Starting Zellij session '$SESSION'..."
script -qfc "zellij --session $SESSION options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
ZELLIJ_PID=$!

# Wait for session
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

sleep 2

# Close floating "about" pane if present (may not appear in all Zellij versions)
zellij action toggle-floating-panes 2>/dev/null || true
sleep 1

# Discover pane ID
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

# Run integration tests
export ZELLIJ_PANE_ID="$PANE_ID"
test_exit=0
/test/scripts/integration-test.sh || test_exit=$?

# Cleanup
zellij kill-session "$SESSION" 2>/dev/null || true
wait $ZELLIJ_PID 2>/dev/null || true

exit $test_exit
