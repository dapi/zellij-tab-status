#!/usr/bin/env bash
#
# Integration tests for zellij-tab-status CLI.
# Must be run inside a Zellij session.
#
# Usage:
#   ZELLIJ_PANE_ID=<id> ./scripts/integration-test.sh
#

set -euo pipefail

PASS=0
FAIL=0
PANE_ID="$ZELLIJ_PANE_ID"

# --- Helpers ---

cli() {
    zellij-tab-status --pane-id "$PANE_ID" "$@"
}

cli_pane() {
    local pane_id="$1"
    shift
    zellij-tab-status --pane-id "$pane_id" "$@"
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

assert_not_contains() {
    local haystack="$1" needle="$2" msg="$3"
    if [[ "$haystack" != *"$needle"* ]]; then
        echo "  PASS: $msg"
        ((PASS++)) || true
    else
        echo "  FAIL: $msg"
        echo "    expected NOT to contain: '$needle'"
        echo "    actual: '$haystack'"
        ((FAIL++)) || true
    fi
}

query_tab_names() {
    zellij action list-tabs 2>/dev/null | head -20 || echo ""
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

wait_for_tab_count() {
    local expected="$1" timeout=10
    local actual start_time
    start_time=$(date +%s)
    while true; do
        actual=$(zellij action list-tabs 2>/dev/null | wc -l)
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
    tab_count=$(zellij action list-tabs 2>/dev/null | wc -l)
    while [[ "$tab_count" -gt 1 ]]; do
        zellij action go-to-tab "$tab_count" 2>/dev/null || true
        sleep 0.2
        zellij action close-tab 2>/dev/null || true
        sleep 0.5
        tab_count=$(zellij action list-tabs 2>/dev/null | wc -l)
    done
    zellij action go-to-tab 1 2>/dev/null || true
    sleep 0.3
}

cleanup() {
    cli --clear 2>/dev/null || true
    close_extra_tabs
}
trap cleanup EXIT

echo "=== zellij-tab-status CLI Integration Tests ==="
echo "Pane ID: $PANE_ID"
echo ""

# --- Test 1: --version ---
echo "--- 1. --version ---"
result=$(zellij-tab-status --version)
if [[ "$result" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "  PASS: --version returns valid version: $result"
    ((PASS++)) || true
else
    echo "  FAIL: --version returned: '$result'"
    ((FAIL++)) || true
fi

# --- Test 2: --help ---
echo "--- 2. --help ---"
result=$(zellij-tab-status --help)
assert_contains "$result" "zellij-tab-status" "--help mentions tool name"

# --- Test 3: set_status + get_status ---
echo "--- 3. set_status / get_status ---"
cli --set-name "TestTab"
sleep 0.3
cli 🧪
sleep 0.3
result=$(cli --get)
assert_eq "$result" "🧪" "get_status returns emoji after set_status"

# --- Test 4: get_name preserves base name ---
echo "--- 4. get_name with status ---"
result=$(cli --name)
assert_eq "$result" "TestTab" "get_name returns base name even with status set"

# --- Test 5: clear_status ---
echo "--- 5. clear_status ---"
cli --clear
sleep 0.3
result=$(cli --get)
assert_eq "$result" "" "get_status returns empty after clear"

# --- Test 6: set_name ---
echo "--- 6. set_name ---"
cli --set-name "Renamed"
sleep 0.3
result=$(cli --name)
assert_eq "$result" "Renamed" "get_name returns new name after set_name"

# --- Test 7: set_name preserves status ---
echo "--- 7. set_name with existing status ---"
cli 🔬
sleep 0.3
cli --set-name "SciTab"
sleep 0.3
result=$(cli --name)
assert_eq "$result" "SciTab" "set_name preserves status, get_name returns new base"
result=$(cli --get)
assert_eq "$result" "🔬" "status preserved after set_name"

# --- Test 8: --get-status alias ---
echo "--- 8. --get-status alias ---"
result=$(cli --get-status)
assert_eq "$result" "🔬" "--get-status works as alias for --get"

# --- Test 9: -g, -c, -n, -s short flags ---
echo "--- 9. Short flags ---"
cli --clear
sleep 0.3
cli -s "ShortTest"
sleep 0.3
result=$(cli -n)
assert_eq "$result" "ShortTest" "-s and -n work"
cli 🎯
sleep 0.3
result=$(cli -g)
assert_eq "$result" "🎯" "-g works"
cli -c
sleep 0.3
result=$(cli -g)
assert_eq "$result" "" "-c clears"

# --- Test 10: No args = get_status ---
echo "--- 10. No args = get_status ---"
cli 🤖
sleep 0.3
result=$(cli)
assert_eq "$result" "🤖" "no args returns status"

# --- Test 11: Multi-tab set_status targets correct tab ---
echo "--- 11. Multi-tab: set_status by pane_id ---"
cli --clear
cli --set-name "TabA"
sleep 0.3

# Create second tab
zellij action new-tab --name "TabB"
sleep 1
zellij action go-to-tab 1
sleep 0.3

# Set status on our tab's pane (should rename TabA, not TabB)
cli 🎯
sleep 0.5

result=$(cli --name)
assert_eq "$result" "TabA" "set_status targets correct tab"

# --- Test 12: --pane-id flag ---
echo "--- 12. --pane-id flag ---"
result=$(zellij-tab-status --pane-id "$PANE_ID" --get)
assert_eq "$result" "🎯" "--pane-id resolves correctly"

# --- Test 13: --tab-id flag ---
echo "--- 13. --tab-id flag ---"
TAB_ID=$(zellij action list-panes --json 2>/dev/null | python3 -c "
import sys, json
panes = json.load(sys.stdin)
for p in panes:
    if p['id'] == $PANE_ID:
        print(p['tab_id'])
        break
" 2>/dev/null || echo "")

if [[ -n "$TAB_ID" ]]; then
    result=$(zellij-tab-status --tab-id "$TAB_ID" --get)
    assert_eq "$result" "🎯" "--tab-id works directly"
else
    echo "  SKIP: Could not determine tab_id (python3 not available?)"
fi

# --- Test 14: Background tab set_status ---
echo "--- 14. Background tab: set_status from another tab ---"
close_extra_tabs
PANE_ID=$(discover_pane_id)
cli --set-name "FG"
cli --clear
sleep 0.3

zellij action new-tab
wait_for_tab_count 2
PANE_BG=$(discover_pane_id)
cli_pane "$PANE_BG" --set-name "BG"
sleep 0.3

# From tab2, set status on tab1
cli_pane "$PANE_ID" 🌟
sleep 0.5
result=$(cli_pane "$PANE_ID" --get)
assert_eq "$result" "🌟" "set_status works from background"
result=$(cli_pane "$PANE_BG" --get)
assert_eq "$result" "" "BG tab untouched"

# --- Test 15: Tab delete + create ---
echo "--- 15. Tab lifecycle: delete + create ---"
close_extra_tabs
PANE_ID=$(discover_pane_id)

cli --set-name "Alpha"
cli --clear
sleep 0.3

zellij action new-tab
wait_for_tab_count 2
PANE_BETA=$(discover_pane_id)
cli_pane "$PANE_BETA" --set-name "Beta"
sleep 0.3

zellij action new-tab
wait_for_tab_count 3
PANE_GAMMA=$(discover_pane_id)
cli_pane "$PANE_GAMMA" --set-name "Gamma"
sleep 0.3

# Delete middle tab
zellij action go-to-tab 2
sleep 0.3
zellij action close-tab
wait_for_tab_count 2

# Verify surviving tabs
result=$(cli_pane "$PANE_ID" --name)
assert_eq "$result" "Alpha" "Alpha survives deletion"
result=$(cli_pane "$PANE_GAMMA" --name)
assert_eq "$result" "Gamma" "Gamma survives deletion"

# Set status on surviving tab
cli_pane "$PANE_GAMMA" 🎉
sleep 0.3
result=$(cli_pane "$PANE_GAMMA" --get)
assert_eq "$result" "🎉" "set_status works after tab deletion"

# --- Test 16: Rapid overwrites ---
echo "--- 16. Rapid set_status overwrite ---"
close_extra_tabs
PANE_ID=$(discover_pane_id)
cli --set-name "Rapid"
cli --clear
sleep 0.3

cli 1️⃣
cli 2️⃣
sleep 0.5
result=$(cli --get)
assert_eq "$result" "2️⃣" "last set_status wins"

# --- Test 17: Exit codes ---
echo "--- 17. Exit codes ---"
# Mutually exclusive flags
set +e
zellij-tab-status --pane-id 1 --tab-id 1 --get 2>/dev/null
exit_code=$?
set -e
assert_eq "$exit_code" "2" "mutually exclusive flags = exit 2"

# Unknown option
set +e
zellij-tab-status --invalid-option 2>/dev/null
exit_code=$?
set -e
assert_eq "$exit_code" "2" "unknown option = exit 2"

# --- Summary ---
echo ""
echo "==============================="
echo "Results: $PASS passed, $FAIL failed"
echo "==============================="
exit $FAIL
