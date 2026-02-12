# Auto-Config Design: Automatic Plugin Registration in config.kdl

## Problem

`make install` copies the WASM plugin to `~/.config/zellij/plugins/` but does not register it in `config.kdl`. Users must manually add the plugin to `load_plugins {}` block, which is easy to miss.

## Solution

Add a bash script `scripts/configure-zellij.sh` that automatically adds the plugin to `~/.config/zellij/config.kdl`. Called from `make install`.

## Behavior

### Happy Path
1. Backup `config.kdl` to `config.kdl.bak`
2. Find `load_plugins {` line
3. Insert plugin path after opening brace
4. Report success

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| Plugin already registered | Skip, print "already configured" |
| `load_plugins {}` exists (empty) | Insert plugin inside |
| `load_plugins { ... }` has other plugins | Add plugin to existing list |
| No `load_plugins` block | Add block at end of file |
| No `config.kdl` exists | Copy default from `zellij setup --dump-config`, then add plugin |
| No `~/.config/zellij/` dir | Create directory first |

### Safety

- Always create backup before modification
- Idempotent: safe to run multiple times
- Validate result: check braces are balanced
- Print diff of changes

## Implementation

### New File: `scripts/configure-zellij.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

PLUGIN_NAME="zellij-tab-status"
PLUGIN_PATH="file:~/.config/zellij/plugins/zellij-tab-status.wasm"
CONFIG_DIR="$HOME/.config/zellij"
CONFIG_FILE="$CONFIG_DIR/config.kdl"

# 1. Ensure config dir exists
# 2. If no config.kdl, dump default
# 3. Check if already configured
# 4. Backup config
# 5. Insert plugin into load_plugins block
# 6. Validate and show diff
```

### Makefile Changes

```makefile
install: build install-scripts configure-zellij
    # ... existing install steps ...

configure-zellij:
    @./scripts/configure-zellij.sh
```

## Plugin Line Format

```kdl
load_plugins {
    "file:~/.config/zellij/plugins/zellij-tab-status.wasm"
}
```

## Rollback

If something goes wrong:
```bash
cp ~/.config/zellij/config.kdl.bak ~/.config/zellij/config.kdl
```

## Testing

Manual testing scenarios:
1. Fresh install (no config.kdl)
2. Empty load_plugins block
3. Existing plugins in load_plugins
4. Already installed (idempotency)
5. Malformed config (should warn, not crash)
