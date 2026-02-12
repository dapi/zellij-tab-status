#!/usr/bin/env bash
#
# configure-zellij.sh - Register zellij-tab-status plugin in config.kdl
#
# Usage: ./scripts/configure-zellij.sh [--uninstall]
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
    cat <<'HELPEOF'
configure-zellij.sh - Register zellij-tab-status plugin in config.kdl

Usage:
  ./scripts/configure-zellij.sh           Add plugin to config
  ./scripts/configure-zellij.sh --help    Show this help

Safety:
  - Creates backup at ~/.config/zellij/config.kdl.bak
  - Idempotent: safe to run multiple times
  - Shows diff of changes
HELPEOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    show_help
    exit 0
fi
