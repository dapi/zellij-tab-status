#!/usr/bin/env bash
#
# Integration tests for zellij-tab-status plugin.
# Must be run inside a Zellij session after `make install`.
#
# Usage:
#   ./scripts/integration-test.sh
#
# Creates a temporary second tab for multi-tab tests, cleans up on exit.

set -euo pipefail

PLUGIN_PATH="${PLUGIN_PATH:-}"
PASS=0
FAIL=0
PANE_ID="$ZELLIJ_PANE_ID"
CREATED_TAB=false

# --- Helpers ---

pipe_cmd() {
    if [[ -n "${PLUGIN_PATH:-}" ]]; then
        timeout 10s zellij pipe --plugin "$PLUGIN_PATH" --name tab-status -- "$1" < /dev/null
    else
        timeout 10s zellij pipe --name tab-status -- "$1" < /dev/null
    fi
}

assert_eq() {
    local actual="$1" expected="$2" msg="$3"
    if [[ "$actual" == "$expected" ]]; then
        echo "  PASS: $msg"
        ((PASS++)) || true
    else
        echo "  FAIL: $msg"
        echo "    expected: '$expected'"
        echo "    actual:   '$actual'"
        ((FAIL++)) || true
    fi
}

assert_contains() {
    local haystack="$1" needle="$2" msg="$3"
    if [[ "$haystack" == *"$needle"* ]]; then
        echo "  PASS: $msg"
        ((PASS++)) || true
    else
        echo "  FAIL: $msg"
        echo "    expected to contain: '$needle'"
        echo "    actual: '$haystack'"
        ((FAIL++)) || true
    fi
}

cleanup() {
    # Remove status from test tab
    pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"clear_status\"}" 2>/dev/null || true
    # Close second tab if created
    if $CREATED_TAB; then
        zellij action go-to-tab 2 2>/dev/null || true
        sleep 0.2
        zellij action close-tab 2>/dev/null || true
        sleep 0.2
    fi
}
trap cleanup EXIT

# --- Setup ---

echo "=== zellij-tab-status Integration Tests ==="
echo "Pane ID: $PANE_ID"
echo ""

pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_name\",\"name\":\"TestTab\"}"
sleep 0.5

# --- Test 1: get_name ---
echo "--- 1. get_name ---"
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_name\"}")
assert_eq "$result" "TestTab" "get_name returns current tab name"

# --- Test 2: set_status + get_status ---
echo "--- 2. set_status / get_status ---"
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_status\",\"emoji\":\"🧪\"}"
sleep 0.3
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_status\"}")
assert_eq "$result" "🧪" "get_status returns emoji after set_status"

# --- Test 3: get_name preserves base name with status ---
echo "--- 3. get_name with status ---"
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_name\"}")
assert_eq "$result" "TestTab" "get_name returns base name even with status set"

# --- Test 4: clear_status ---
echo "--- 4. clear_status ---"
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"clear_status\"}"
sleep 0.3
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_status\"}")
assert_eq "$result" "" "get_status returns empty after clear_status"

# Verify tab name is clean
tab_names=$(zellij action query-tab-names)
assert_contains "$tab_names" "TestTab" "query-tab-names shows clean name"

# --- Test 5: set_name ---
echo "--- 5. set_name ---"
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_name\",\"name\":\"Renamed\"}"
sleep 0.3
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_name\"}")
assert_eq "$result" "Renamed" "get_name returns new name after set_name"

# --- Test 6: set_name preserves status ---
echo "--- 6. set_name with existing status ---"
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_status\",\"emoji\":\"🔬\"}"
sleep 0.3
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_name\",\"name\":\"SciTab\"}"
sleep 0.3
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_name\"}")
assert_eq "$result" "SciTab" "set_name preserves status, get_name returns new base"
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_status\"}")
assert_eq "$result" "🔬" "status preserved after set_name"

# Clean up for multi-tab tests
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"clear_status\"}"
sleep 0.2

# --- Test 7: Multi-tab mapping (KEY TEST for Bug #1) ---
echo "--- 7. Multi-tab: pane maps to correct tab ---"

pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_name\",\"name\":\"TabA\"}"
sleep 0.3

# Create a second tab — focus moves there, but our pane stays in TabA
zellij action new-tab --name "TabB"
CREATED_TAB=true
sleep 1

# Go back to our tab
zellij action go-to-tab 1
sleep 0.5

# Our pane should still map to TabA, NOT TabB
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_name\"}")
assert_eq "$result" "TabA" "pane maps to OWN tab, not neighbor (Bug #1 regression)"

# --- Test 8: set_status renames correct tab, not focused (KEY TEST for Bug #2) ---
echo "--- 8. set_status renames correct tab via plugin API ---"
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_status\",\"emoji\":\"🎯\"}"
sleep 0.5

tab_names=$(zellij action query-tab-names)
echo "  Tab names after set_status: $tab_names"

# TabA should have the status, TabB should be untouched
assert_contains "$tab_names" "🎯 TabA" "TabA has status emoji"
assert_contains "$tab_names" "TabB" "TabB is untouched"

# --- Test 9: get_version ---
echo "--- 9. get_version ---"
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_version\"}")
# Should be a semver-like string
if [[ "$result" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "  PASS: get_version returns valid version: $result"
    ((PASS++)) || true
else
    echo "  FAIL: get_version returned unexpected: '$result'"
    ((FAIL++)) || true
fi

# --- Summary ---
echo ""
echo "==============================="
echo "Results: $PASS passed, $FAIL failed"
echo "==============================="
exit $FAIL
