#!/usr/bin/env bash
#
# Regression check for issue #5:
# "After first --plugin usage, floating panel becomes empty/unresponsive".
#
# Runs INSIDE Docker container with:
# - empty load_plugins config (on-demand plugin loading)
# - first call via `zellij pipe --plugin ...`
# - then `toggle-floating-panes` + attempted input
#
# Fails if typed input does not execute.
#
set -euo pipefail

PLUGIN_WASM="/test/plugin.wasm"
PLUGIN_PATH="file:$PLUGIN_WASM"
ATTEMPTS="${ISSUE5_ATTEMPTS:-3}"

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

failures=0

run_once() {
    local run="$1"
    local session="issue5-regression-$run"
    local zpid=""
    local base_file="/tmp/issue5_base_$run"
    local probe_file="/tmp/issue5_probe_$run"

    cleanup() {
        zellij kill-session "$session" >/dev/null 2>&1 || true
        if [[ -n "$zpid" ]]; then
            wait "$zpid" >/dev/null 2>&1 || true
        fi
        unset ZELLIJ_SESSION
    }
    trap cleanup RETURN

    rm -f "$base_file" "$probe_file"

    script -qfc "zellij --session $session options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
    zpid=$!

    for _ in $(seq 1 30); do
        if zellij list-sessions 2>/dev/null | grep -q "$session"; then
            break
        fi
        if ! kill -0 "$zpid" 2>/dev/null; then
            echo "run_$run: session died before start"
            return 1
        fi
        sleep 0.3
    done

    if ! zellij list-sessions 2>/dev/null | grep -q "$session"; then
        echo "run_$run: session did not start"
        return 1
    fi

    export ZELLIJ_SESSION="$session"
    sleep 1

    zellij action write-chars "echo \$ZELLIJ_PANE_ID > $base_file"
    zellij action write 13
    sleep 0.7

    local base_pane
    base_pane="$(tr -d '[:space:]' < "$base_file")"
    if [[ -z "$base_pane" ]]; then
        echo "run_$run: failed to discover base pane id"
        return 1
    fi

    local payload
    payload="{\"pane_id\":\"$base_pane\",\"action\":\"get_version\"}"
    timeout 5s zellij pipe --plugin "$PLUGIN_PATH" -- "$payload" < /dev/null > /dev/null || true
    sleep 0.5

    # The issue manifests when user opens floating panes after first plugin usage.
    zellij action toggle-floating-panes > /dev/null 2>&1 || true
    sleep 0.5

    zellij action write-chars "echo issue5_ok > $probe_file"
    zellij action write 13
    sleep 0.7

    if [[ -f "$probe_file" ]]; then
        echo "run_$run: input=OK"
        return 0
    fi

    echo "run_$run: input=FAIL"
    return 1
}

for run in $(seq 1 "$ATTEMPTS"); do
    if ! run_once "$run"; then
        failures=$((failures + 1))
    fi
done

echo "summary attempts=$ATTEMPTS failures=$failures"

if [[ "$failures" -gt 0 ]]; then
    echo "FAIL: issue #5 reproduced"
    exit 1
fi

echo "PASS: issue #5 not reproduced"
