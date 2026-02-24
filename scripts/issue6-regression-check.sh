#!/usr/bin/env bash
#
# Regression check for issue #6:
# "In a fresh session, first zellij-tab-status '>' call is ignored."
#
# Runs INSIDE Docker container with on-demand plugin loading (empty load_plugins).
# Verifies CLI wrapper retries on NOT_READY and applies status on first user call.
#
set -euo pipefail

PLUGIN_WASM="/test/plugin.wasm"
INSTALLED_WASM="/root/.config/zellij/plugins/zellij-tab-status.wasm"
SCRIPT_PATH="/test/scripts/zellij-tab-status"
ATTEMPTS="${ISSUE6_ATTEMPTS:-3}"

if [[ ! -f "$PLUGIN_WASM" ]]; then
    echo "ERROR: Plugin not found at $PLUGIN_WASM"
    exit 1
fi
if [[ ! -x "$SCRIPT_PATH" ]]; then
    echo "ERROR: Script not executable at $SCRIPT_PATH"
    exit 1
fi

mkdir -p /root/.config/zellij /root/.config/zellij/plugins /root/.cache/zellij
cp "$PLUGIN_WASM" "$INSTALLED_WASM"
cat > /root/.config/zellij/config.kdl <<EOF
show_startup_tips false
show_release_notes false
default_layout "compact"
EOF

cat > /root/.cache/zellij/permissions.kdl <<EOF
"$PLUGIN_WASM" {
    ReadApplicationState
    ChangeApplicationState
    ReadCliPipes
}
"$INSTALLED_WASM" {
    ReadApplicationState
    ChangeApplicationState
    ReadCliPipes
}
EOF

has_status_marker() {
    local names="$1"
    printf '%s\n' "$names" | grep -Eq '(^|[[:space:]])>[[:space:]]'
}

failures=0

run_once() {
    local run="$1"
    local session="issue6-regression-$run"
    local zpid=""
    local pane_file="/tmp/issue6_pane_$run"
    local pane_id=""

    cleanup() {
        zellij kill-session "$session" >/dev/null 2>&1 || true
        if [[ -n "$zpid" ]]; then
            wait "$zpid" >/dev/null 2>&1 || true
        fi
        unset ZELLIJ_SESSION
        unset ZELLIJ_PANE_ID
        unset ZELLIJ
    }
    trap cleanup RETURN

    rm -f "$pane_file"

    script -qfc "zellij --session $session options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
    zpid=$!

    for _ in $(seq 1 40); do
        if zellij list-sessions 2>/dev/null | grep -q "$session"; then
            break
        fi
        if ! kill -0 "$zpid" 2>/dev/null; then
            echo "run_$run: session died before start"
            return 1
        fi
        sleep 0.2
    done

    if ! zellij list-sessions 2>/dev/null | grep -q "$session"; then
        echo "run_$run: session did not start"
        return 1
    fi

    export ZELLIJ_SESSION="$session"
    sleep 0.8

    for _ in $(seq 1 20); do
        rm -f "$pane_file"
        zellij action write-chars "echo \$ZELLIJ_PANE_ID > $pane_file" >/dev/null 2>&1 || true
        zellij action write 13 >/dev/null 2>&1 || true
        sleep 0.25
        pane_id="$(tr -d '[:space:]' < "$pane_file" 2>/dev/null || true)"
        if [[ -n "$pane_id" ]]; then
            break
        fi
        sleep 0.1
    done

    if [[ -z "$pane_id" ]]; then
        echo "run_$run: failed to discover pane id"
        return 1
    fi

    local before after_first
    before="$(zellij action query-tab-names 2>/dev/null || true)"

    export ZELLIJ_PANE_ID="$pane_id"
    export ZELLIJ=1
    timeout 5s "$SCRIPT_PATH" ">" > /dev/null || true
    sleep 0.35
    after_first="$(zellij action query-tab-names 2>/dev/null || true)"

    printf 'run_%s before:\n%s\n' "$run" "$before"
    printf 'run_%s after_first:\n%s\n' "$run" "$after_first"

    if ! has_status_marker "$after_first"; then
        echo "run_$run: reproduced (first user call did not apply status)"
        return 1
    fi

    echo "run_$run: OK"
    return 0
}

for run in $(seq 1 "$ATTEMPTS"); do
    if ! run_once "$run"; then
        failures=$((failures + 1))
    fi
done

echo "summary attempts=$ATTEMPTS failures=$failures"

if [[ "$failures" -gt 0 ]]; then
    echo "FAIL: issue #6 reproduced"
    exit 1
fi

echo "PASS: issue #6 not reproduced"
