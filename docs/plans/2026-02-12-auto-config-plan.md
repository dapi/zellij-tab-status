# Auto-Config Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Automatically register zellij-tab-status plugin in config.kdl during `make install`

**Architecture:** Bash script `configure-zellij.sh` handles all edge cases (no config, empty block, existing plugins), creates backup, inserts plugin line via sed, validates result. Called from Makefile.

**Tech Stack:** Bash, sed, diff

---

## Task 1: Create configure-zellij.sh with basic structure

**Files:**
- Create: `scripts/configure-zellij.sh`

**Step 1: Create the script with constants and help**

```bash
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
```

**Step 2: Make executable**

```bash
chmod +x scripts/configure-zellij.sh
```

**Step 3: Test help works**

Run: `./scripts/configure-zellij.sh --help`
Expected: Shows usage info without errors

**Step 4: Commit**

```bash
git add scripts/configure-zellij.sh
git commit -m "feat: add configure-zellij.sh skeleton"
```

---

## Task 2: Add config directory and file creation

**Files:**
- Modify: `scripts/configure-zellij.sh`

**Step 1: Add ensure_config_exists function**

Append after `show_help` function:

```bash
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
```

**Step 2: Add main execution block at end of file**

```bash
# Main
ensure_config_dir
ensure_config_file

log "Config file: $CONFIG_FILE"
```

**Step 3: Test with existing config**

Run: `./scripts/configure-zellij.sh`
Expected: Shows "Config file: /home/.../.config/zellij/config.kdl"

**Step 4: Commit**

```bash
git add scripts/configure-zellij.sh
git commit -m "feat: add config dir/file creation"
```

---

## Task 3: Add idempotency check

**Files:**
- Modify: `scripts/configure-zellij.sh`

**Step 1: Add check_already_configured function**

Add after `ensure_config_file`:

```bash
check_already_configured() {
    if grep -q "zellij-tab-status.wasm" "$CONFIG_FILE"; then
        log "Plugin already configured in $CONFIG_FILE"
        exit 0
    fi
}
```

**Step 2: Call it in main block**

Update main block:

```bash
# Main
ensure_config_dir
ensure_config_file
check_already_configured

log "Config file: $CONFIG_FILE"
```

**Step 3: Test idempotency**

First, manually add plugin line to config, then run:
Run: `./scripts/configure-zellij.sh`
Expected: "Plugin already configured" and exit 0

**Step 4: Remove test line and commit**

```bash
git add scripts/configure-zellij.sh
git commit -m "feat: add idempotency check"
```

---

## Task 4: Add backup functionality

**Files:**
- Modify: `scripts/configure-zellij.sh`

**Step 1: Add backup function**

Add after `check_already_configured`:

```bash
create_backup() {
    cp "$CONFIG_FILE" "$BACKUP_FILE"
    log "Backup created: $BACKUP_FILE"
}
```

**Step 2: Call in main block**

```bash
# Main
ensure_config_dir
ensure_config_file
check_already_configured
create_backup

log "Config file: $CONFIG_FILE"
```

**Step 3: Test backup creation**

Run: `./scripts/configure-zellij.sh`
Expected: "Backup created: ..." and file exists

Run: `ls -la ~/.config/zellij/config.kdl.bak`
Expected: File exists with same size as config.kdl

**Step 4: Commit**

```bash
git add scripts/configure-zellij.sh
git commit -m "feat: add backup before modification"
```

---

## Task 5: Add plugin insertion logic

**Files:**
- Modify: `scripts/configure-zellij.sh`

**Step 1: Add insert_plugin function**

Add after `create_backup`:

```bash
insert_plugin() {
    # Check if load_plugins block exists
    if grep -q "^load_plugins {" "$CONFIG_FILE"; then
        # Insert after "load_plugins {"
        sed -i '/^load_plugins {/a\'"$PLUGIN_LINE" "$CONFIG_FILE"
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
```

**Step 2: Call in main and show diff**

Update main block:

```bash
# Main
ensure_config_dir
ensure_config_file
check_already_configured
create_backup
insert_plugin

# Show what changed
log "Changes made:"
diff "$BACKUP_FILE" "$CONFIG_FILE" || true

log "Done! Restart zellij session to load plugin."
```

**Step 3: Test insertion**

Run: `./scripts/configure-zellij.sh`
Expected: Shows diff with added plugin line

Run: `grep -A2 "load_plugins" ~/.config/zellij/config.kdl`
Expected: Shows load_plugins block with our plugin

**Step 4: Commit**

```bash
git add scripts/configure-zellij.sh
git commit -m "feat: add plugin insertion logic"
```

---

## Task 6: Add validation

**Files:**
- Modify: `scripts/configure-zellij.sh`

**Step 1: Add validate function**

Add after `insert_plugin`:

```bash
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
```

**Step 2: Call after insert**

Update main:

```bash
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
```

**Step 3: Test validation**

Run: `./scripts/configure-zellij.sh` (will say already configured if run before)

To test fresh:
```bash
cp ~/.config/zellij/config.kdl.bak ~/.config/zellij/config.kdl
./scripts/configure-zellij.sh
```
Expected: "Validation passed"

**Step 4: Commit**

```bash
git add scripts/configure-zellij.sh
git commit -m "feat: add config validation with auto-rollback"
```

---

## Task 7: Update Makefile

**Files:**
- Modify: `Makefile`

**Step 1: Add configure-zellij target**

Add new target and update install dependency:

Change line 11 from:
```makefile
install: build install-scripts
```

To:
```makefile
install: build install-scripts configure-zellij
```

Add new target after `install-scripts`:

```makefile
configure-zellij:
	@./scripts/configure-zellij.sh || echo "⚠️  Config update failed, add plugin manually"
```

**Step 2: Update .PHONY**

Change line 1 from:
```makefile
.PHONY: build install install-scripts clean
```

To:
```makefile
.PHONY: build install install-scripts configure-zellij clean
```

**Step 3: Remove manual instructions from install target**

Remove lines 19-22 (the echo statements about adding to config.kdl):
```makefile
	@echo "📝 Add to ~/.config/zellij/config.kdl:"
	@echo '  load_plugins {'
	@echo '      "file:$(INSTALL_DIR)/$(PLUGIN_NAME).wasm"'
	@echo '  }'
```

**Step 4: Test full install**

```bash
# Restore config to test
cp ~/.config/zellij/config.kdl.bak ~/.config/zellij/config.kdl
make install
```

Expected: Builds, installs, configures automatically

**Step 5: Commit**

```bash
git add Makefile
git commit -m "feat: integrate configure-zellij into make install"
```

---

## Task 8: Update documentation

**Files:**
- Modify: `README.md`

**Step 1: Update installation section**

Find "Configure Zellij" section (around line 42) and update to note it's automatic now.

After "make install" in Option 2, add note:

```markdown
The installer automatically configures `~/.config/zellij/config.kdl`.
```

Remove or comment out the manual config.kdl instructions since they're no longer needed.

**Step 2: Add troubleshooting for auto-config**

In Troubleshooting section, add:

```markdown
### Auto-Config Failed

If automatic configuration fails:
1. Check backup: `~/.config/zellij/config.kdl.bak`
2. Manually add to `~/.config/zellij/config.kdl`:
   ```kdl
   load_plugins {
       "file:~/.config/zellij/plugins/zellij-tab-status.wasm"
   }
   ```
3. Report issue: https://github.com/dapi/zellij-tab-status/issues
```

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: update installation for auto-config"
```

---

## Task 9: Final testing

**Step 1: Test fresh install scenario**

```bash
# Remove all traces
rm -f ~/.config/zellij/plugins/zellij-tab-status.wasm
# Restore original config (remove our plugin line)
cp ~/.config/zellij/config.kdl.bak ~/.config/zellij/config.kdl

# Full install
make clean && make install
```

Expected: Plugin installed and configured

**Step 2: Test idempotency**

```bash
make install
make install
```

Expected: Second run says "already configured", no errors

**Step 3: Test in zellij**

Restart zellij session and check:
```bash
tail -f /tmp/zellij-$(id -u)/zellij-log/zellij.log | grep tab-status
```

Expected: "[tab-status] Plugin loaded"

**Step 4: Final commit**

```bash
git add -A
git commit -m "test: verify auto-config works end-to-end" --allow-empty
```
