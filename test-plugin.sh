#!/bin/bash
# Test script for zellij-tab-status plugin
# Run inside a Zellij session: ./test-plugin.sh

set -e

LOG_FILE="/tmp/zellij-$(id -u)/zellij-log/zellij.log"

echo "=== Testing zellij-tab-status plugin ==="
echo "ZELLIJ_PANE_ID: $ZELLIJ_PANE_ID"
echo ""

if [[ -z "$ZELLIJ_PANE_ID" ]]; then
    echo "ERROR: Not running inside Zellij session"
    exit 1
fi

# Wait for plugin to load
sleep 1

show_tabs() {
    echo "Current tabs:"
    zellij action query-tab-names
    echo ""
}

# --- Test 1: set_status ---
echo "=== Test 1: set_status ==="
zellij pipe --name tab-status -- "{\"pane_id\": \"$ZELLIJ_PANE_ID\", \"action\": \"set_status\", \"emoji\": \"🤖\"}"
sleep 0.5
show_tabs

# --- Test 2: get_status ---
echo "=== Test 2: get_status ==="
echo -n "Status emoji: "
zellij pipe --name tab-status -- "{\"pane_id\": \"$ZELLIJ_PANE_ID\", \"action\": \"get_status\"}"
echo ""

# --- Test 3: get_name ---
echo "=== Test 3: get_name ==="
echo -n "Base name: "
zellij pipe --name tab-status -- "{\"pane_id\": \"$ZELLIJ_PANE_ID\", \"action\": \"get_name\"}"
echo ""

# --- Test 4: change status ---
echo "=== Test 4: change status (🤖 → ✅) ==="
zellij pipe --name tab-status -- "{\"pane_id\": \"$ZELLIJ_PANE_ID\", \"action\": \"set_status\", \"emoji\": \"✅\"}"
sleep 0.5
show_tabs

# --- Test 5: complex emoji (flag) ---
echo "=== Test 5: complex emoji (flag 🇺🇸) ==="
zellij pipe --name tab-status -- "{\"pane_id\": \"$ZELLIJ_PANE_ID\", \"action\": \"set_status\", \"emoji\": \"🇺🇸\"}"
sleep 0.5
show_tabs

# --- Test 6: clear_status ---
echo "=== Test 6: clear_status ==="
zellij pipe --name tab-status -- "{\"pane_id\": \"$ZELLIJ_PANE_ID\", \"action\": \"clear_status\"}"
sleep 0.5
show_tabs

# --- Test 7: legacy tab-rename ---
echo "=== Test 7: legacy tab-rename ==="
zellij pipe --name tab-rename -- "{\"pane_id\": \"$ZELLIJ_PANE_ID\", \"name\": \"Legacy-Test\"}"
sleep 0.5
show_tabs

# --- Cleanup ---
echo "=== Cleanup: clear status ==="
zellij pipe --name tab-status -- "{\"pane_id\": \"$ZELLIJ_PANE_ID\", \"action\": \"clear_status\"}"
sleep 0.5
show_tabs

echo "=== Recent plugin logs ==="
grep "tab-status" "$LOG_FILE" | tail -20 || echo "No logs found"

echo ""
echo "=== All tests completed ==="
