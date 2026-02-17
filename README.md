# zellij-tab-status

[![CI](https://github.com/dapi/zellij-tab-status/actions/workflows/ci.yml/badge.svg)](https://github.com/dapi/zellij-tab-status/actions/workflows/ci.yml)

Zellij plugin for managing tab status with emoji prefixes.

## Features

- **Set/clear emoji status** on any tab by pane_id
- **Rename tabs** without losing the emoji status prefix
- **Query current status**, base name, or plugin version programmatically
- **Atomic operations** — no race conditions when updating status
- **Unicode-aware** — handles complex emoji (flags 🇺🇸, skin tones 👋🏻, ZWJ sequences 👨‍👩‍👧)

## Installation

### Option 1: Download Release (Recommended)

```bash
# Download latest release
curl -L https://github.com/dapi/zellij-tab-status/releases/latest/download/zellij-tab-status.wasm \
  -o ~/.config/zellij/plugins/zellij-tab-status.wasm
```

### Option 2: Build from Source

```bash
# Prerequisites
rustup target add wasm32-wasip1

# Build & Install
git clone https://github.com/dapi/zellij-tab-status
cd zellij-tab-status
make install
```

### Configure Zellij

`make install` automatically configures `~/.config/zellij/config.kdl`.

Restart Zellij session to load the plugin.

## Usage

### Basic Status Management

```bash
# Set status emoji: "my-tab" → "🤖 my-tab"
zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "set_status", "emoji": "🤖"}'

# Change status: "🤖 my-tab" → "✅ my-tab"
zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "set_status", "emoji": "✅"}'

# Clear status: "✅ my-tab" → "my-tab"
zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "clear_status"}'
```

### Rename Tab (Preserving Status)

```bash
# Rename tab without losing emoji: "🤖 my-tab" → "🤖 new-name"
zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "set_name", "name": "new-name"}'
```

### Query

```bash
# Get current emoji (outputs to stdout)
zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_status"}'
# Output: 🤖

# Get base name without emoji
zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_name"}'
# Output: my-tab

# Get installed plugin version
zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_version"}'
# Output: 0.4.0
```

## Status Emoji Examples

| Status | Emoji | Use Case |
|--------|-------|----------|
| Working | 🤖 | Processing task |
| Waiting | ⏳ | Long operation |
| Input needed | ✋ | Requires user input |
| Success | ✅ | Task completed |
| Error | ❌ | Task failed |
| Warning | ⚠️ | Attention needed |
| Building | 🔨 | Compilation |
| Testing | 🧪 | Running tests |
| Deploying | 🚀 | Deployment in progress |

## CLI Scripts

Ready-to-use wrapper scripts are included in `scripts/`:

### Install scripts

```bash
# Copy to ~/.local/bin (or anywhere in PATH)
cp scripts/zellij-tab-status ~/.local/bin/
chmod +x ~/.local/bin/zellij-tab-status
```

### Usage

```bash
zellij-tab-status 🤖           # Set status emoji
zellij-tab-status ⏳           # Change status
zellij-tab-status --clear      # Remove status
zellij-tab-status              # Get current emoji
zellij-tab-status --name       # Get base name
zellij-tab-status -s "Code"   # Rename tab (preserving emoji)
zellij-tab-status --version    # Get plugin version
```

### Shell aliases (optional)

Add to `~/.bashrc` or `~/.zshrc`:

```bash
alias ts='zellij-tab-status'
alias tsc='zellij-tab-status --clear'
alias tsn='zellij-tab-status --name'
alias tsr='zellij-tab-status --set-name'
```

## Integration Examples

### Show Status During Long Commands

```bash
# Wrapper for long-running commands
with-status() {
    local emoji="${1:-🤖}"
    shift
    tab-status "$emoji"
    "$@"
    local exit_code=$?
    if [[ $exit_code -eq 0 ]]; then
        tab-status ✅
    else
        tab-status ❌
    fi
    return $exit_code
}

# Usage
with-status 🔨 make build
with-status 🧪 npm test
with-status 🚀 ./deploy.sh
```

### Git Hook Integration

```bash
# .git/hooks/pre-commit
#!/bin/bash
tab-status 🔍
# ... run checks ...
```

### CI/CD Status Display

```bash
#!/bin/bash
tab-status 🚀
if deploy_to_staging; then
    tab-status ✅
    echo "Deploy successful"
else
    tab-status ❌
    echo "Deploy failed"
    exit 1
fi
```

### Claude Code Integration

This plugin works with [zellij-tab-claude-status](https://github.com/dapi/claude-code-marketplace/tree/master/zellij-tab-claude-status) — a Claude Code plugin that shows AI session state in Zellij tabs:

- 🟢 Ready — waiting for input
- 🤖 Working — processing request
- ✋ Needs input — permission prompt waiting

## API Reference

### `tab-status` Pipe

JSON payload with `pane_id` and `action`:

| Action | Required Fields | Description |
|--------|-----------------|-------------|
| `set_status` | `emoji` | Set emoji prefix on tab |
| `clear_status` | — | Remove emoji prefix |
| `get_status` | — | Output current emoji to stdout |
| `get_name` | — | Output base name (without emoji) to stdout |
| `set_name` | `name` | Set tab name, preserving emoji prefix |
| `get_version` | — | Output plugin version to stdout |

### Status Format

Status = first grapheme cluster + space.

| Tab Name | Status | Base Name |
|----------|--------|-----------|
| `🤖 Working` | `🤖` | `Working` |
| `🇺🇸 USA` | `🇺🇸` | `USA` |
| `Working` | `` (empty) | `Working` |

## Troubleshooting

### Check Plugin Logs

```bash
tail -f /tmp/zellij-$(id -u)/zellij-log/zellij.log | grep tab-status
```

### Plugin Not Responding

1. Verify plugin is loaded: run `zellij-tab-status --version` or check Zellij logs for `[tab-status] Plugin loaded`
2. Check `$ZELLIJ_PANE_ID` is set (only works inside Zellij)
3. Restart Zellij session after config changes

### Wrong Tab Updated

Plugin maps `pane_id` → tab. If you have multiple panes in a tab, any pane_id from that tab will update the same tab name.

### Unicode Issues

Plugin uses grapheme clustering. If emoji appears broken:
- Ensure terminal supports Unicode
- Check font has emoji glyphs
- Try simpler emoji (🟢 instead of 👨‍👩‍👧)

### Auto-Config Failed

If automatic configuration fails during `make install`:
1. Check backup: `~/.config/zellij/config.kdl.bak`
2. Manually add to `~/.config/zellij/config.kdl`:
   ```kdl
   load_plugins {
       "file:~/.config/zellij/plugins/zellij-tab-status.wasm"
   }
   ```
3. Report issue: https://github.com/dapi/zellij-tab-status/issues

## Development

```bash
# Build
make build

# Install locally
make install

# Clean
make clean

# Run unit tests
make test

# Test in live Zellij session (after make install)
make test-live
```

## License

MIT
