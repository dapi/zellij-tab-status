# CLAUDE.md

## Project Overview

**zellij-tab-status** — Native Rust CLI tool for Zellij terminal multiplexer. Manages tab status with emoji prefixes using `zellij action` commands. Requires Zellij main branch (pinned commit `a8d99b64a3`).

## Tech Stack

- **Language:** Rust
- **Target:** Native binary (no WASM)
- **Dependencies:** serde, serde_json, unicode-segmentation
- **Zellij API:** `zellij action` CLI commands (`list-panes --json`, `list-tabs --json`, `rename-tab-by-id`)
- **Testing:** Docker + Zellij (built from source) for integration tests

## Build Commands

```bash
# Build native binary
make build

# Install to ~/.local/bin/
make install

# Clean build artifacts
make clean

# Run unit tests (tab_name module, 46 tests)
make test

# Run integration tests (Docker required)
make test-integration
```

## Project Structure

```
zellij-tab-status/
├── Cargo.toml              # Package config, dependencies
├── Cargo.lock              # Locked versions
├── Makefile                # Build/install/test targets
├── Dockerfile.test         # Docker image (builds Zellij from source)
├── README.md               # User documentation
├── src/
│   ├── main.rs             # CLI entry point, arg parsing, orchestration
│   ├── lib.rs              # Library root (module exports)
│   ├── tab_name.rs         # Tab name parsing with U+2063 marker (+ 46 tests)
│   └── zellij_api.rs       # Subprocess calls to zellij CLI
├── scripts/
│   ├── integration-test.sh     # Integration test cases (17 groups)
│   └── docker-test-runner.sh   # Starts headless Zellij in Docker, runs tests
└── .github/workflows/
    ├── ci.yml              # CI: lint, unit tests, build, integration tests
    └── release.yml         # Release: build + GitHub Release on tag push
```

## Architecture

### CLI-First Design

Stateless read-modify-write cycle per invocation:
1. Resolve `pane_id` → `tab_id` via `zellij action list-panes --json`
2. Get tab name via `zellij action list-tabs --json`
3. Parse/modify name using `tab_name` module
4. Rename via `zellij action rename-tab-by-id <tab_id> <name>`

### Tab Name Format (U+2063 Marker)

Status is stored as: `U+2063 + emoji + SPACE + base_name`

- U+2063 (INVISIBLE SEPARATOR) is an unambiguous marker — never appears in user-typed names
- `tab_name::get_status()` / `get_name()` / `set_status()` / `clear_status()` / `set_name()` — pure functions
- `tab_name::first_grapheme()` extracts first grapheme cluster (handles flag emoji, skin tones, ZWJ sequences)

### Zellij API (zellij_api.rs)

Three functions wrapping `std::process::Command`:
- `resolve_tab_id(pane_id) -> Result<u32, String>` — `zellij action list-panes --json`
- `get_tab_name(tab_id) -> Result<String, String>` — `zellij action list-tabs --json`
- `rename_tab(tab_id, new_name) -> Result<(), String>` — `zellij action rename-tab-by-id`

### Tab ID Resolution

Precedence: `--tab-id` > `--pane-id` > `$ZELLIJ_PANE_ID` (mutually exclusive, exit 2 on conflict).

### Unicode Handling

Uses `unicode-segmentation` for proper emoji handling:
- Flag emoji: 🇺🇸 (2 code points, 1 grapheme)
- Skin tones: 👋🏻 (2 code points, 1 grapheme)
- ZWJ sequences: 👨‍👩‍👧 (multiple code points, 1 grapheme)

## Code Conventions

- Error handling: `eprintln!("Error: ...")` + `process::exit(2)` for user errors, `process::exit(1)` for runtime errors
- Pure logic in `tab_name.rs`, side effects in `main.rs` and `zellij_api.rs`
- No panics — all errors handled gracefully with exit codes

### NEVER use `zellij action rename-tab`

**ЗАПРЕЩЕНО** использовать `zellij action rename-tab`. Эта команда переименовывает FOCUSED (активную) вкладку, а не конкретную. Используйте `zellij action rename-tab-by-id <tab_id> <name>`.

## Testing

```bash
# Unit tests (46 tests in tab_name module):
cargo test --lib

# Integration tests (Docker required, builds Zellij from source):
make test-integration
```

### Integration Test Architecture

`make test-integration` runs:
1. `cargo build --release` — build native binary
2. `docker build -f Dockerfile.test` — Ubuntu + Zellij (from source at pinned commit)
3. `docker run` with mounted binary + scripts:
   - `docker-test-runner.sh` starts headless Zellij via `script` (PTY), discovers pane ID, runs tests
   - `integration-test.sh` executes 17 test groups via CLI binary

Key details:
- Zellij needs PTY: `script -qfc "zellij ..." /dev/null > /dev/null 2>&1 &`
- Pane ID discovered via `zellij action write-chars 'echo $ZELLIJ_PANE_ID > /tmp/pane_id'`

## Related Projects

- **zellij-tab-claude-status** — Claude Code plugin that uses this CLI tool
  - Repository: github.com/dapi/claude-code-marketplace
  - Uses `zellij-tab-status` CLI for session state indicators
