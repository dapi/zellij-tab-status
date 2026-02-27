#!/usr/bin/env bash
#
# Regression check for issue #6 (plugin-side queueing):
# A single raw `zellij pipe --plugin ... set_status` call in a fresh session
# must eventually apply, even if it arrives before pane mapping is ready.
#
set -euo pipefail

PLUGIN_WASM="/test/plugin.wasm"
PLUGIN_PATH="file:$PLUGIN_WASM"
ATTEMPTS="${ISSUE6_RAW_ATTEMPTS:-3}"

if [[ ! -f "$PLUGIN_WASM" ]]; then
    echo "ERROR: Plugin not found at $PLUGIN_WASM"
    exit 1
fi

mkdir -p /root/.config/zellij /root/.cache/zellij
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
EOF

has_status_marker() {
    local names="$1"
    printf '%s\n' "$names" | grep -Eq '(^|[[:space:]])>[[:space:]]'
}

has_probe_marker() {
    local names="$1"
    printf '%s\n' "$names" | grep -q '⍟'
}

failures=0

run_once() {
    local run="$1"
    local session="issue6-raw-regression-$run"
    local zpid=""
    local pane_file="/tmp/issue6_raw_pane_$run"
    local pane_id=""

    cleanup() {
        zellij kill-session "$session" >/dev/null 2>&1 || true
        if [[ -n "$zpid" ]]; then
            wait "$zpid" >/dev/null 2>&1 || true
        fi
        unset ZELLIJ_SESSION
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

    local before after payload
    before="$(zellij action query-tab-names 2>/dev/null || true)"
    payload="{\"pane_id\":\"$pane_id\",\"action\":\"set_status\",\"emoji\":\">\"}"

    timeout 5s zellij pipe --plugin "$PLUGIN_PATH" -- "$payload" < /dev/null > /dev/null || true

    after="$before"
    for _ in $(seq 1 50); do
        sleep 0.2
        after="$(zellij action query-tab-names 2>/dev/null || true)"
        if has_status_marker "$after" && ! has_probe_marker "$after"; then
            break
        fi
    done

    printf 'run_%s before:\n%s\n' "$run" "$before"
    printf 'run_%s after:\n%s\n' "$run" "$after"

    if ! has_status_marker "$after" || has_probe_marker "$after"; then
        echo "run_$run: reproduced (single raw call did not apply status)"
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

max_allowed=$(( (ATTEMPTS - 1) / 2 ))
if [[ "$failures" -gt "$max_allowed" ]]; then
    echo "FAIL: issue #6 raw regression reproduced ($failures/$ATTEMPTS failed, max allowed=$max_allowed)"
    exit 1
fi

echo "PASS: issue #6 raw regression not reproduced ($failures/$ATTEMPTS transient failures within tolerance)"
