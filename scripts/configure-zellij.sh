#!/usr/bin/env bash
#
# configure-zellij.sh - Register zellij-tab-status plugin in config.kdl
#
# Usage: ./scripts/configure-zellij.sh
#
# Safely adds plugin to load_plugins block with backup.

set -euo pipefail

PLUGIN_NAME="zellij-tab-status"
PLUGIN_LINE='    "file:~/.config/zellij/plugins/zellij-tab-status.wasm"'
CONFIG_DIR="$HOME/.config/zellij"
CONFIG_FILE="$CONFIG_DIR/config.kdl"
BACKUP_FILE="$CONFIG_FILE.bak"

log() { echo "[configure-zellij] $*"; }
error() { echo "[configure-zellij] ERROR: $*" >&2; }

show_help() {
    cat <<'EOF'
configure-zellij.sh - Register zellij-tab-status plugin in config.kdl

Usage:
  ./scripts/configure-zellij.sh           Add plugin to config
  ./scripts/configure-zellij.sh --help    Show this help

Safety:
  - Creates backup at ~/.config/zellij/config.kdl.bak
  - Idempotent: safe to run multiple times
  - Shows diff of changes
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    show_help
    exit 0
fi

ensure_config_dir() {
    if [[ ! -d "$CONFIG_DIR" ]]; then
        log "Creating $CONFIG_DIR"
        mkdir -p "$CONFIG_DIR"
    fi
}

ensure_config_file() {
    if [[ ! -f "$CONFIG_FILE" ]]; then
        log "No config.kdl found, creating from zellij defaults..."
        if command -v zellij &>/dev/null; then
            zellij setup --dump-config > "$CONFIG_FILE"
            log "Created $CONFIG_FILE from zellij defaults"
        else
            error "zellij not found in PATH, cannot create default config"
            error "Please create $CONFIG_FILE manually"
            exit 1
        fi
    fi
}

check_already_configured() {
    if grep -q "zellij-tab-status.wasm" "$CONFIG_FILE"; then
        log "Plugin already configured in $CONFIG_FILE"
        exit 0
    fi
}

create_backup() {
    cp "$CONFIG_FILE" "$BACKUP_FILE"
    log "Backup created: $BACKUP_FILE"
}

insert_plugin() {
    # Check if load_plugins block exists
    if grep -q "^[[:space:]]*load_plugins {" "$CONFIG_FILE"; then
        # Insert after "load_plugins {" (cross-platform: use temp file)
        local tmpfile
        tmpfile=$(mktemp)
        sed '/^[[:space:]]*load_plugins {/a\'"$PLUGIN_LINE" "$CONFIG_FILE" > "$tmpfile"
        mv "$tmpfile" "$CONFIG_FILE"
        log "Added plugin to existing load_plugins block"
    else
        # Append new block at end
        cat >> "$CONFIG_FILE" <<EOF

load_plugins {
$PLUGIN_LINE
}
EOF
        log "Added new load_plugins block with plugin"
    fi
}

validate_config() {
    # Simple brace balance check
    local open_braces close_braces
    open_braces=$(grep -o '{' "$CONFIG_FILE" | wc -l)
    close_braces=$(grep -o '}' "$CONFIG_FILE" | wc -l)

    if [[ "$open_braces" -ne "$close_braces" ]]; then
        error "Validation failed: unbalanced braces ($open_braces open, $close_braces close)"
        error "Restoring backup..."
        cp "$BACKUP_FILE" "$CONFIG_FILE"
        exit 1
    fi

    # Check plugin line is present
    if ! grep -q "zellij-tab-status.wasm" "$CONFIG_FILE"; then
        error "Validation failed: plugin line not found after insertion"
        error "Restoring backup..."
        cp "$BACKUP_FILE" "$CONFIG_FILE"
        exit 1
    fi

    log "Validation passed"
}

# Main
ensure_config_dir
ensure_config_file
check_already_configured
create_backup
insert_plugin
validate_config

# Show what changed
log "Changes made:"
diff "$BACKUP_FILE" "$CONFIG_FILE" || true

log "Done! Restart zellij session to load plugin."
