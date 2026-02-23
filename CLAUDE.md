# CLAUDE.md

## Project Overview

**zellij-tab-status** — Rust WASM plugin for Zellij terminal multiplexer. Manages tab status with emoji prefixes.

## Tech Stack

- **Language:** Rust
- **Target:** `wasm32-wasip1` (WebAssembly)
- **Framework:** `zellij-tile` 0.43.1
- **Dependencies:** serde, serde_json, unicode-segmentation
- **Testing:** Docker + Zellij 0.43.1 for integration tests

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

# Run integration tests (Docker required, no Zellij session needed)
make test-integration

# Test in live Zellij session
make test-live
```

## Project Structure

```
zellij-tab-status/
├── Cargo.toml              # Package config, dependencies
├── Cargo.lock              # Locked versions
├── Makefile                # Build/install/test targets
├── Dockerfile.test         # Docker image for integration tests (Ubuntu + Zellij)
├── README.md               # User documentation
├── src/
│   ├── main.rs             # Plugin entry point (Zellij API calls)
│   ├── lib.rs              # Library root (module exports)
│   ├── pipe_handler.rs     # Pipe command handlers (pure logic + tests)
│   └── status_utils.rs     # Unicode emoji/status extraction (+ tests)
├── scripts/
│   ├── zellij-tab-status       # CLI: manage tab status emoji
│   ├── integration-test.sh     # Integration test cases (runs inside Zellij)
│   └── docker-test-runner.sh   # Starts headless Zellij in Docker, runs tests
└── .github/workflows/
    ├── ci.yml              # CI: lint, unit tests, build, integration tests
    └── release.yml         # Release: build + GitHub Release on tag push
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
{"action": "get_version"}
{"action": "get_debug"}
```

Note: `get_version` and `get_debug` do not require `pane_id`. `get_debug` returns JSON with `tab_indices`, `next_tab_index`, `pane_tab_index`, `pane_to_tab_count` — handled in main.rs before pipe_handler.

### Plugin Loading

The plugin is loaded **on-demand** via `zellij pipe --plugin "file:path.wasm"`. Do NOT add it to `load_plugins` in config.kdl — this creates duplicate instances when CLI also uses `--plugin`.

**CRITICAL: Never use `--plugin` and `--name` together in `zellij pipe`.** Zellij routes the message via both paths independently, causing the plugin to receive it **twice** (double output, double side effects). Use only one:
- `--plugin "file:path.wasm"` — targets a specific plugin (auto-loads if needed). Used by the CLI script.
- `--name tab-status` — broadcasts to all plugins with that name. Used by integration tests in Docker where the plugin is pre-loaded via `load_plugins`.

For integration tests in Docker, the plugin IS pre-loaded via `load_plugins` in a test-specific config, and pipe commands use `--name` only (no `--plugin`).

### State Management

- `pane_to_tab: PaneTabMap` (alias for `BTreeMap<u32, (usize, String)>`) — maps pane_id to (tab_position, tab_name). Rebuilt on every `TabUpdate` or `PaneUpdate` event
- `pending_renames: BTreeMap<usize, String>` — protects against `rebuild_mapping` overwriting inline cache updates with stale tab names before `TabUpdate` confirms a rename
- `tab_indices: Vec<u32>` — maps tab position → persistent Zellij tab index (workaround for Zellij bug #3535)
- `next_tab_index: u32` — counter for assigning indices to newly created tabs
- `pane_tab_index: HashMap<u32, u32>` — maps pane_id → persistent tab_index. Pane IDs are stable anchors for identifying tabs across structural changes (deletions/creations)

### Zellij Bug #3535: rename_tab uses persistent tab index

**CRITICAL**: `rename_tab(tab_position, name)` in `zellij-tile` shim is named misleadingly.
The Zellij server treats the value as a **persistent internal tab index**, NOT a position.

- Issue: https://github.com/zellij-org/zellij/issues/3535
- Fix PR (NOT merged as of Zellij 0.43.1): https://github.com/zellij-org/zellij/pull/4179

**Behavior:**
- Tab indices are 1-indexed, assigned sequentially at creation (1, 2, 3, ...)
- Indices are NEVER reassigned after deletion. Deleting tab index 1 leaves [2, 3, ...]
- `TabInfo.position` IS re-indexed after deletion (0, 1, 2, ...) — so `position + 1 != index` after any deletion

**Workaround — three-level pipeline:**

1. **pipe_handler.rs** (pure logic) — receives `pane_id`, looks up `tab_position` in `pane_to_tab` cache, returns `PipeEffect::RenameTab { tab_position, name }`. Knows nothing about persistent indices.

2. **main.rs: get_tab_index(position)** — converts position → persistent index via `tab_indices[position]`. Called when executing `RenameTab` effects. This is the ONLY place where position→index conversion happens.

3. **main.rs: update_tab_indices()** — maintains the `tab_indices` vector using pane-ID anchors:
   - On first `TabUpdate`: assumes `[1..=N]`
   - On structural change (tab count changed): looks up surviving panes via `pane_tab_index` (pane_id → index). Panes found in `pane_tab_index` with a valid current index keep their old index; unknown panes get `next_tab_index++`
   - Filters stale entries: only trusts `pane_tab_index` values present in current `tab_indices` (prevents reused pane IDs from mapping to deleted tab indices)

**Critical timing constraint:** `sync_pane_tab_index()` must NOT run in `PaneUpdate` handler — only inside `update_tab_indices()`. Reason: `PaneUpdate` can arrive BEFORE `TabUpdate` during tab deletion, when pane positions have already shifted but `tab_indices` is stale. Syncing at that moment would corrupt `pane_tab_index` (mapping surviving panes to wrong indices).

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

### NEVER use `zellij action rename-tab`

**ЗАПРЕЩЕНО** использовать `zellij action rename-tab` в скриптах и врапперах. Эта команда переименовывает FOCUSED (активную) вкладку, а не конкретную. Скрипт должен работать из ЛЮБОЙ вкладки, даже если она не в фокусе.

Все переименования табов ТОЛЬКО через plugin API `rename_tab(tab_id, name)` внутри WASM-плагина. CLI-скрипт отправляет команду через `zellij pipe`, плагин сам вызывает `rename_tab()` с правильным `tab_id`, определённым по `pane_id`.

## Testing

```bash
# Unit tests (39 tests in pipe_handler + status_utils, no WASM runtime needed):
cargo test --lib

# Integration tests (101 assertions in 19 tests, Docker required, runs headless Zellij):
make test-integration

# In Zellij session (after make install + restart):
make test-live

# Check logs:
tail -f /tmp/zellij-1000/zellij-log/zellij.log | grep tab-status
```

### Integration Test Architecture

`make test-integration` runs:
1. `cargo build --release --target wasm32-wasip1` — build fresh .wasm
2. `docker build -f Dockerfile.test` — Ubuntu + Zellij image
3. `docker run` with mounted .wasm + scripts:
   - `docker-test-runner.sh` creates Zellij config + permissions, starts headless session via `script` (PTY), discovers pane ID, runs tests
   - `integration-test.sh` executes 19 test groups (101 assertions) via `zellij pipe`

Key details for Docker testing:
- Zellij needs PTY: `script -qfc "zellij ..." /dev/null > /dev/null 2>&1 &`
- Permissions pre-approved in `~/.cache/zellij/permissions.kdl` (no UI in headless)
- Pane ID discovered via `zellij action write-chars 'echo $ZELLIJ_PANE_ID > /tmp/pane_id'`
- WASM compile takes ~3s; `sleep 5` after session start

## Related Projects

- **zellij-tab-claude-status** — Claude Code plugin that uses this Zellij plugin
  - Repository: github.com/dapi/claude-code-marketplace
  - Uses `tab-status` pipe for session state indicators
