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

discover_pane_id() {
    local tmp_file="/tmp/pane_id_discover_$$"
    zellij action write-chars "echo \$ZELLIJ_PANE_ID > $tmp_file"
    zellij action write 13
    sleep 1
    local result
    result=$(cat "$tmp_file" 2>/dev/null | tr -d '[:space:]')
    rm -f "$tmp_file" 2>/dev/null
    echo "$result"
}

# Polling helpers: wait for expected state instead of fixed sleep

wait_for_name() {
    local pane_id="$1" expected="$2" msg="$3" timeout=10
    local actual start_time
    start_time=$(date +%s)
    while true; do
        actual=$(pipe_cmd "{\"pane_id\":\"$pane_id\",\"action\":\"get_name\"}" 2>/dev/null) || true
        if [[ "$actual" == "$expected" ]]; then
            echo "  PASS: $msg"
            ((PASS++)) || true
            return 0
        fi
        if [[ $(( $(date +%s) - start_time )) -ge $timeout ]]; then
            echo "  FAIL: $msg (timeout ${timeout}s)"
            echo "    expected: '$expected'"
            echo "    actual:   '$actual'"
            ((FAIL++)) || true
            return 1
        fi
        sleep 0.3
    done
}

wait_for_status() {
    local pane_id="$1" expected="$2" msg="$3" timeout=10
    local actual start_time
    start_time=$(date +%s)
    while true; do
        actual=$(pipe_cmd "{\"pane_id\":\"$pane_id\",\"action\":\"get_status\"}" 2>/dev/null) || true
        if [[ "$actual" == "$expected" ]]; then
            echo "  PASS: $msg"
            ((PASS++)) || true
            return 0
        fi
        if [[ $(( $(date +%s) - start_time )) -ge $timeout ]]; then
            echo "  FAIL: $msg (timeout ${timeout}s)"
            echo "    expected: '$expected'"
            echo "    actual:   '$actual'"
            ((FAIL++)) || true
            return 1
        fi
        sleep 0.3
    done
}

wait_for_tab_contains() {
    local needle="$1" msg="$2" timeout=10
    local actual start_time
    start_time=$(date +%s)
    while true; do
        actual=$(zellij action query-tab-names 2>/dev/null) || true
        if [[ "$actual" == *"$needle"* ]]; then
            echo "  PASS: $msg"
            ((PASS++)) || true
            return 0
        fi
        if [[ $(( $(date +%s) - start_time )) -ge $timeout ]]; then
            echo "  FAIL: $msg (timeout ${timeout}s)"
            echo "    expected to contain: '$needle'"
            echo "    actual: '$actual'"
            ((FAIL++)) || true
            return 1
        fi
        sleep 0.3
    done
}

wait_for_tab_count() {
    local expected="$1" timeout=10
    local actual start_time
    start_time=$(date +%s)
    while true; do
        actual=$(zellij action query-tab-names 2>/dev/null | wc -l)
        if [[ "$actual" -eq "$expected" ]]; then
            return 0
        fi
        if [[ $(( $(date +%s) - start_time )) -ge $timeout ]]; then
            echo "  WARNING: tab count timeout (expected $expected, got $actual)"
            return 1
        fi
        sleep 0.3
    done
}

close_extra_tabs() {
    local tab_count
    tab_count=$(zellij action query-tab-names 2>/dev/null | wc -l)
    while [[ "$tab_count" -gt 1 ]]; do
        zellij action go-to-tab "$tab_count" 2>/dev/null || true
        sleep 0.2
        zellij action close-tab 2>/dev/null || true
        sleep 0.5
        tab_count=$(zellij action query-tab-names 2>/dev/null | wc -l)
    done
    zellij action go-to-tab 1 2>/dev/null || true
    sleep 0.3
}

cleanup() {
    pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"clear_status\"}" 2>/dev/null || true
    close_extra_tabs
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

# --- Test 10: Multi-tab lifecycle (create, verify, delete, re-create) ---
echo "--- 10. Multi-tab lifecycle ---"

# Phase 1: Setup 3 tabs
echo "  Phase 1: Setup 3 tabs"
close_extra_tabs

# Tab1 already exists, rename to Alpha
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"clear_status\"}"
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_name\",\"name\":\"Alpha\"}"
wait_for_name "$PANE_ID" "Alpha" "tab1 renamed to Alpha"

# Create tab2 (Beta)
zellij action new-tab
wait_for_tab_count 2
PANE_ID_2=$(discover_pane_id)
echo "  Discovered PANE_ID_2=$PANE_ID_2"
pipe_cmd "{\"pane_id\":\"$PANE_ID_2\",\"action\":\"set_name\",\"name\":\"Beta\"}"
wait_for_name "$PANE_ID_2" "Beta" "tab2 renamed to Beta"

# Create tab3 (Gamma)
zellij action new-tab
wait_for_tab_count 3
PANE_ID_3=$(discover_pane_id)
echo "  Discovered PANE_ID_3=$PANE_ID_3"
pipe_cmd "{\"pane_id\":\"$PANE_ID_3\",\"action\":\"set_name\",\"name\":\"Gamma\"}"
wait_for_name "$PANE_ID_3" "Gamma" "tab3 renamed to Gamma"

# Phase 2: Extra pane in tab1
echo "  Phase 2: Extra pane in tab1"
zellij action go-to-tab 1
sleep 0.3
zellij action new-pane
sleep 1
PANE_ID_1B=$(discover_pane_id)
echo "  Discovered PANE_ID_1B=$PANE_ID_1B"
wait_for_name "$PANE_ID_1B" "Alpha" "second pane in tab1 also returns Alpha"

# Phase 3: Verify all 3 tabs
echo "  Phase 3: Verify all 3 tabs"
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"get_name\"}")
assert_eq "$result" "Alpha" "tab1 pane1 is Alpha"

result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID_2\",\"action\":\"get_name\"}")
assert_eq "$result" "Beta" "tab2 is Beta"

result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID_3\",\"action\":\"get_name\"}")
assert_eq "$result" "Gamma" "tab3 is Gamma"

pipe_cmd "{\"pane_id\":\"$PANE_ID_2\",\"action\":\"set_status\",\"emoji\":\"🟢\"}"
wait_for_status "$PANE_ID_2" "🟢" "Beta has green status"
wait_for_tab_contains "🟢 Beta" "query-tab-names has 🟢 Beta"

tab_names=$(zellij action query-tab-names)
assert_contains "$tab_names" "Alpha" "query-tab-names has Alpha"
assert_contains "$tab_names" "Gamma" "query-tab-names has Gamma"

# Phase 4: Delete tab1
echo "  Phase 4: Delete tab1 (Alpha)"
zellij action go-to-tab 1
sleep 0.3
zellij action close-tab
wait_for_tab_count 2

# Phase 5: Verify remaining 2 tabs after deletion
echo "  Phase 5: Verify after tab1 deletion"
wait_for_name "$PANE_ID_2" "Beta" "Beta still accessible after tab1 deleted"
wait_for_name "$PANE_ID_3" "Gamma" "Gamma still accessible after tab1 deleted"
wait_for_status "$PANE_ID_2" "🟢" "Beta status preserved after tab1 deleted"

pipe_cmd "{\"pane_id\":\"$PANE_ID_3\",\"action\":\"set_name\",\"name\":\"GammaNew\"}"
wait_for_name "$PANE_ID_3" "GammaNew" "Gamma renamed to GammaNew"

# Phase 6: Create new tab (Delta)
echo "  Phase 6: Create new tab (Delta)"
zellij action new-tab
wait_for_tab_count 3
PANE_ID_4=$(discover_pane_id)
echo "  Discovered PANE_ID_4=$PANE_ID_4"
pipe_cmd "{\"pane_id\":\"$PANE_ID_4\",\"action\":\"set_name\",\"name\":\"Delta\"}"
wait_for_name "$PANE_ID_4" "Delta" "new tab renamed to Delta"

# Phase 7: Final verification of all 3 tabs
echo "  Phase 7: Final verification"
result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID_2\",\"action\":\"get_name\"}")
assert_eq "$result" "Beta" "Beta still correct after new tab"

result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID_3\",\"action\":\"get_name\"}")
assert_eq "$result" "GammaNew" "GammaNew still correct after new tab"

result=$(pipe_cmd "{\"pane_id\":\"$PANE_ID_4\",\"action\":\"get_name\"}")
assert_eq "$result" "Delta" "Delta accessible"

pipe_cmd "{\"pane_id\":\"$PANE_ID_4\",\"action\":\"set_status\",\"emoji\":\"🔵\"}"
wait_for_tab_contains "🟢 Beta" "final: 🟢 Beta present"
wait_for_tab_contains "GammaNew" "final: GammaNew present"
wait_for_tab_contains "🔵 Delta" "final: 🔵 Delta present"

# --- Summary ---
echo ""
echo "==============================="
echo "Results: $PASS passed, $FAIL failed"
echo "==============================="
exit $FAIL
