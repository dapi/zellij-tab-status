# zellij-tab-status

[![CI](https://github.com/dapi/zellij-tab-status/actions/workflows/ci.yml/badge.svg)](https://github.com/dapi/zellij-tab-status/actions/workflows/ci.yml)

CLI tool for managing tab status with emoji prefixes in Zellij.

## Features

- **Set/clear emoji status** on any tab
- **Rename tabs** without losing the emoji status prefix
- **Query current status**, base name, or version programmatically
- **Direct CLI** — no WASM plugin, no pipe protocol, just a binary
- **Unicode-aware** — handles complex emoji (flags, skin tones, ZWJ sequences)

## Requirements

- **Zellij from main branch** (pinned commit with `list-clients` support)
  - The `list-clients` subcommand is required for pane-to-tab resolution
  - Not yet available in any stable release

## Installation

### Build from Source

```bash
# Prerequisites: Rust toolchain
git clone https://github.com/dapi/zellij-tab-status
cd zellij-tab-status
make install
```

This builds the binary and copies it to `~/.local/bin/zellij-tab-status`.

### Manual Install

```bash
cargo build --release
cp target/release/zellij-tab-status ~/.local/bin/
```

## Usage

```bash
# Set status emoji: "my-tab" -> "🤖 my-tab"
zellij-tab-status 🤖

# Get current status emoji
zellij-tab-status --get
zellij-tab-status        # same as --get

# Clear status: "🤖 my-tab" -> "my-tab"
zellij-tab-status --clear

# Get base tab name (without status)
zellij-tab-status --name

# Set tab name (preserving status): "🤖 old" -> "🤖 Build"
zellij-tab-status --set-name "Build"

# Use explicit pane/tab ID
zellij-tab-status --pane-id 7 🤖
zellij-tab-status --tab-id 3 --clear

# Version
zellij-tab-status --version
```

### Shell Aliases (optional)

Add to `~/.bashrc` or `~/.zshrc`:

```bash
alias ts='zellij-tab-status'
alias tsc='zellij-tab-status --clear'
alias tsn='zellij-tab-status --name'
alias tsr='zellij-tab-status --set-name'
```

## Status Emoji Examples

| Status | Emoji | Use Case |
|-|-|-|
| Working | 🤖 | Processing task |
| Waiting | ⏳ | Long operation |
| Input needed | ✋ | Requires user input |
| Success | ✅ | Task completed |
| Error | ❌ | Task failed |
| Warning | ⚠️ | Attention needed |
| Building | 🔨 | Compilation |
| Testing | 🧪 | Running tests |
| Deploying | 🚀 | Deployment in progress |

## Integration Examples

### Show Status During Long Commands

```bash
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
```

### Claude Code Integration

This tool works with [zellij-tab-claude-status](https://github.com/dapi/claude-code-marketplace/tree/master/zellij-tab-claude-status) — a Claude Code plugin that shows AI session state in Zellij tabs:

- 🟢 Ready — waiting for input
- 🤖 Working — processing request
- ✋ Needs input — permission prompt waiting

## How It Works

The tool is a native Rust binary that:

1. Resolves the current tab via `$ZELLIJ_PANE_ID` (or explicit `--pane-id`/`--tab-id`)
2. Reads the current tab name via `zellij action query-tab-names`
3. Manipulates the emoji prefix using unicode grapheme segmentation
4. Renames the tab via `zellij action rename-tab`

Tab names use an invisible U+2063 marker to distinguish status-prefixed names from user-set names.

## Development

```bash
# Build
make build

# Install locally
make install

# Run unit tests
make test
cargo test --lib

# Integration tests (Docker required)
make test-integration
```

## Alternatives

| | zellij-tab-status | [zellaude](https://github.com/ishefi/zellaude) | [zellij-attention](https://github.com/KiryuuLight/zellij-attention) |
|-|-|-|-|
| Type | CLI tool | WASM tab bar | WASM name modifier |
| Tab bar compatibility | Any | Replaces default | Any |
| Works after tab deletion | Yes | Yes | No |
| Status format | `🤖 Name` (prefix) | Custom UI with colors | `Name ⏳` (suffix) |
| Status types | Any emoji | Detailed (tool, thinking) | 2 (waiting/completed) |

## License

MIT
