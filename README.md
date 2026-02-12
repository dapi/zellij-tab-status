# zellij-tab-status

Zellij plugin for managing tab status with emoji prefixes.

## Features

- **Set/clear emoji status** on any tab by pane_id
- **Query current status** or base name programmatically
- **Atomic operations** — no race conditions when updating status
- **Unicode-aware** — handles complex emoji (flags 🇺🇸, skin tones 👋🏻, ZWJ sequences 👨‍👩‍👧)

## Installation

### Prerequisites

```bash
# Install Rust wasm target
rustup target add wasm32-wasip1
```

### Build & Install

```bash
git clone https://github.com/dapi/zellij-tab-status
cd zellij-tab-status
make install
```

### Configure Zellij

Add to `~/.config/zellij/config.kdl`:

```kdl
load_plugins {
    "file:~/.config/zellij/plugins/zellij-tab-status.wasm"
}
```

Restart Zellij session.

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

### Query Status

```bash
# Get current emoji (outputs to stdout)
zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_status"}'
# Output: 🤖

# Get base name without emoji
zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_name"}'
# Output: my-tab
```

### Direct Tab Rename (Legacy)

```bash
# Rename tab completely (use tab-status for status management instead)
zellij pipe --name tab-rename -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "name": "New Name"}'
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

## Bash Wrapper Functions

Add to your `~/.bashrc` or `~/.zshrc`:

```bash
# Set tab status emoji
tab-status() {
    local emoji="$1"
    if [[ -z "$emoji" ]]; then
        # Get current status
        zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_status"}'
    else
        zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "set_status", "emoji": "'"$emoji"'"}'
    fi
}

# Clear tab status
tab-status-clear() {
    zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "clear_status"}'
}

# Get tab base name
tab-name() {
    zellij pipe --name tab-status -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_name"}'
}
```

Usage:

```bash
tab-status 🤖        # Set working status
tab-status ✅        # Set success status
tab-status-clear     # Remove status
tab-status           # Get current emoji
tab-name             # Get base name
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

### `tab-rename` Pipe (Legacy)

JSON payload with `pane_id` and `name`:

```json
{"pane_id": "123", "name": "New Tab Name"}
```

Renames tab completely. Use `tab-status` for status emoji management.

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

1. Verify plugin is loaded: check Zellij logs for `[tab-status] Plugin loaded`
2. Check `$ZELLIJ_PANE_ID` is set (only works inside Zellij)
3. Restart Zellij session after config changes

### Wrong Tab Updated

Plugin maps `pane_id` → tab. If you have multiple panes in a tab, any pane_id from that tab will update the same tab name.

### Unicode Issues

Plugin uses grapheme clustering. If emoji appears broken:
- Ensure terminal supports Unicode
- Check font has emoji glyphs
- Try simpler emoji (🟢 instead of 👨‍👩‍👧)

## Development

```bash
# Build
make build

# Install locally
make install

# Clean
make clean

# Run tests (in Zellij session)
./test-plugin.sh
```

## License

MIT
