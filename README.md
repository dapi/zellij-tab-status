# zellij-tab-status

[![CI](https://github.com/dapi/zellij-tab-status/actions/workflows/ci.yml/badge.svg)](https://github.com/dapi/zellij-tab-status/actions/workflows/ci.yml)

Zellij plugin for managing tab status with emoji prefixes.

## Features

- **Set/clear emoji status** on any tab by pane_id
- **Built-in blinking/rotation** via multi-symbol `set_status` (no external shell loop)
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

No `config.kdl` changes are required for the default setup.
The plugin is launched on-demand via `zellij pipe --plugin ...`.

Optional preloaded mode (if you want `--name tab-status` commands):
```kdl
load_plugins {
    "file:~/.config/zellij/plugins/zellij-tab-status.wasm"
}
```

## Usage

### Basic Status Management

```bash
PLUGIN_PATH="file:$HOME/.config/zellij/plugins/zellij-tab-status.wasm"

# Set status emoji: "my-tab" → "🤖 my-tab"
zellij pipe --plugin "$PLUGIN_PATH" -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "set_status", "emoji": "🤖"}'

# Change status: "🤖 my-tab" → "✅ my-tab"
zellij pipe --plugin "$PLUGIN_PATH" -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "set_status", "emoji": "✅"}'

# Built-in blinking: rotates between frames every 500ms (default)
zellij pipe --plugin "$PLUGIN_PATH" -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "set_status", "emoji": "🔴🟡"}'

# Override blink delay to 350ms
zellij pipe --plugin "$PLUGIN_PATH" -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "set_status", "emoji": "🟥🟨🟩", "delay_ms": 350}'

# Clear status: "✅ my-tab" → "my-tab"
zellij pipe --plugin "$PLUGIN_PATH" -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "clear_status"}'
```

### Rename Tab (Preserving Status)

```bash
# Rename tab without losing emoji: "🤖 my-tab" → "🤖 new-name"
zellij pipe --plugin "$PLUGIN_PATH" -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "set_name", "name": "new-name"}'
```

### Query

```bash
# Get current emoji (outputs to stdout)
zellij pipe --plugin "$PLUGIN_PATH" -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_status"}'
# Output: 🤖

# Get base name without emoji
zellij pipe --plugin "$PLUGIN_PATH" -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_name"}'
# Output: my-tab

# Get installed plugin version
zellij pipe --plugin "$PLUGIN_PATH" -- '{"pane_id": "'$ZELLIJ_PANE_ID'", "action": "get_version"}'
# Output: 0.7.1
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
    zellij-tab-status "$emoji"
    "$@"
    local exit_code=$?
    if [[ $exit_code -eq 0 ]]; then
        zellij-tab-status ✅
    else
        zellij-tab-status ❌
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
zellij-tab-status 🔍
# ... run checks ...
```

### CI/CD Status Display

```bash
#!/bin/bash
zellij-tab-status 🚀
if deploy_to_staging; then
    zellij-tab-status ✅
    echo "Deploy successful"
else
    zellij-tab-status ❌
    echo "Deploy failed"
    exit 1
fi
```

### Claude Code Integration

This plugin works with [zellij-tab-claude-status](https://github.com/dapi/claude-code-marketplace/tree/master/zellij-tab-claude-status) — a Claude Code plugin that shows AI session state in Zellij tabs:

- 🟢 Ready — waiting for input
- 🤖 Working — processing request
- ✋ Needs input — permission prompt waiting

## Alternatives

Other Zellij plugins for Claude Code integration:

| | zellij-tab-status | zellaude | zellij-attention |
|---|---|---|---|
| **Тип** | Модификатор имён | Tab bar replacement | Модификатор имён |
| **Совместимость с tab bar** | ✅ Любой | ❌ Полная замена | ✅ Любой |
| **Баг #3535 решён** | ✅ Probing + pane anchors | — (не трогает табы) | ❌ |
| **Работает после удаления табов** | ✅ | ✅ | ❌ |
| **Формат статуса** | `🤖 Name` (prefix) | Свой UI с цветами | `Name ⏳` (suffix) |
| **Типы статусов** | 🔧 Любой emoji | 🎨 Детальные (tool, thinking, etc.) | 2 (waiting/completed) |
| **Настройки** | ❌ | ✅ (UI меню) | ✅ (через layout) |

**zellij-tab-status** — базовый слой, совместим с любым tab bar (zjstatus, стандартный, кастомный).

**zellaude** — "всё-в-одном" для тех, кто хочет готовый красивый UI.

**zellij-attention** — минималист, только waiting/completed.

## API Reference

### `tab-status` Pipe

JSON payload with `pane_id` and `action`:

| Action | Required Fields | Description |
|--------|-----------------|-------------|
| `set_status` | `emoji` | Set status prefix on tab. 1 grapheme = static, 2+ graphemes = blink/rotate |
| `clear_status` | — | Remove emoji prefix |
| `get_status` | — | Output current emoji to stdout |
| `get_name` | — | Output base name (without emoji) to stdout |
| `set_name` | `name` | Set tab name, preserving emoji prefix |
| `get_version` | — | Output plugin version to stdout |

`set_status` optional field:

- `delay_ms` — blink/rotation interval in milliseconds for multi-grapheme statuses (default `500`)

### Status Format

Status = first grapheme cluster + space.

If `set_status.emoji` contains multiple grapheme clusters, only one grapheme is shown at a time and frames rotate on the configured timer.

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

### Preloaded Mode (Optional)

If you want to target plugin by name (`--name tab-status`) instead of `--plugin`,
add the plugin to `~/.config/zellij/config.kdl`:
```kdl
load_plugins {
    "file:~/.config/zellij/plugins/zellij-tab-status.wasm"
}
```

## Development

```bash
# Build
make build

# Install locally
make install

# Clean
make clean

# Run host tests (plain `cargo test` should be green)
# Note: binary test harness is disabled in Cargo.toml because
# zellij-tile host symbols are only available inside Zellij runtime.
cargo test
make test

# Test in live Zellij session (after make install)
make test-live

# Integration tests (mode: preloaded|ondemand)
make test-integration
TEST_PLUGIN_MODE=ondemand make test-integration

# Verify on-demand --plugin does not duplicate plugin instance
make test-plugin-dedup

# Regression check for issue #5 (floating panel input)
make test-issue5-regression
```

## License

MIT
