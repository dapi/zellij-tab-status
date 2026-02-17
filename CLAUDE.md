# CLAUDE.md

## Project Overview

**zellij-tab-status** — Rust WASM plugin for Zellij terminal multiplexer. Manages tab status with emoji prefixes.

## Tech Stack

- **Language:** Rust
- **Target:** `wasm32-wasip1` (WebAssembly)
- **Framework:** `zellij-tile` 0.43.1
- **Dependencies:** serde, serde_json, unicode-segmentation

## Build Commands

```bash
# Build WASM plugin
make build

# Install to ~/.config/zellij/plugins/
make install

# Clean build artifacts
make clean

# Run unit tests (no WASM runtime needed)
make test

# Test in live Zellij session
make test-live
```

## Project Structure

```
zellij-tab-status/
├── Cargo.toml            # Package config, dependencies
├── Cargo.lock            # Locked versions
├── Makefile              # Build/install targets
├── README.md             # User documentation
├── src/
│   ├── main.rs           # Plugin entry point (Zellij API calls)
│   ├── lib.rs            # Library root (module exports)
│   ├── pipe_handler.rs   # Pipe command handlers (pure logic + tests)
│   └── status_utils.rs   # Unicode emoji/status extraction (+ tests)
└── scripts/
    └── zellij-tab-status   # CLI: manage tab status emoji
```

## Architecture

### Zellij Plugin API

Plugin uses `zellij-tile` crate:
- `register_plugin!(State)` — registers plugin state
- `ZellijPlugin` trait — lifecycle hooks (load, update, pipe, render)
- `Event::TabUpdate`, `Event::PaneUpdate` — track tab/pane state
- `PipeMessage` — receive commands from CLI

### Pipe Commands

All commands go through `tab-status` pipe:
```json
{"pane_id": "123", "action": "set_status", "emoji": "🤖"}
{"pane_id": "123", "action": "clear_status"}
{"pane_id": "123", "action": "get_status"}
{"pane_id": "123", "action": "get_name"}
{"pane_id": "123", "action": "set_name", "name": "New Name"}
{"pane_id": "123", "action": "get_version"}
```

### State Management

- `pane_to_tab: PaneTabMap` (alias for `BTreeMap<u32, (usize, String)>`) — maps pane_id to (tab_position, tab_name)
- Rebuilt on every `TabUpdate` or `PaneUpdate` event
- Tab position is 0-indexed internally (from `TabInfo.position`), converted to 1-indexed `tab_id` in `pipe_handler.rs` for the `rename_tab()` API

### Unicode Handling

Uses `unicode-segmentation` for proper emoji handling:
- Flag emoji: 🇺🇸 (2 code points, 1 grapheme)
- Skin tones: 👋🏻 (2 code points, 1 grapheme)
- Status = first grapheme + space

## Code Conventions

- Log prefix: `[tab-status]` for all eprintln! calls
- Error handling: handlers return `Vec<PipeEffect>` (empty on error), main.rs executes effects
- "Functional core, imperative shell": pure handlers in `pipe_handler.rs`, side effects in `main.rs`
- `unblock_cli_pipe_input()` called after CLI pipe handling to prevent CLI hang (only for `PipeSource::Cli`)
- No panics — all errors logged and gracefully handled

## Testing

```bash
# Unit tests (pipe_handler + status_utils, no WASM runtime needed):
cargo test --lib

# In Zellij session (after make install + restart):
make test-live

# Check logs:
tail -f /tmp/zellij-1000/zellij-log/zellij.log | grep tab-status
```

## Related Projects

- **zellij-tab-claude-status** — Claude Code plugin that uses this Zellij plugin
  - Repository: github.com/dapi/claude-code-marketplace
  - Uses `tab-status` pipe for session state indicators
