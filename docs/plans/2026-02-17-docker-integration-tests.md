# Docker Integration Tests Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Run integration tests inside a Docker container with a real Zellij session, both locally (`make test-integration`) and in GitHub Actions CI.

**Architecture:** Docker container with Zellij binary + mounted freshly-built .wasm plugin. A wrapper script starts Zellij headlessly (via `script` for PTY), discovers the pane ID, runs the existing integration-test.sh, and exits with its result code. The same Docker image is used locally and in CI.

**Tech Stack:** Docker, Zellij 0.43.1, bash, GitHub Actions

---

### Task 1: Create Dockerfile.test

**Files:**
- Create: `Dockerfile.test`

**Step 1: Write the Dockerfile**

```dockerfile
FROM ubuntu:24.04

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       curl ca-certificates util-linux \
    && curl -L https://github.com/zellij-org/zellij/releases/download/v0.43.1/zellij-x86_64-unknown-linux-musl.tar.gz \
       | tar xz -C /usr/local/bin \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /test
```

Key details:
- `util-linux` provides `script` command for pseudo-TTY
- Zellij 0.43.1 matches the version used in development
- No Rust toolchain needed — .wasm is pre-built and mounted

**Step 2: Build the image to verify it works**

Run: `docker build -f Dockerfile.test -t zellij-tab-status-test .`
Expected: image builds successfully, `docker run --rm zellij-tab-status-test zellij --version` prints `zellij 0.43.1`

**Step 3: Commit**

```bash
git add Dockerfile.test
git commit -m "feat: add Dockerfile.test for integration testing"
```

---

### Task 2: Create docker-test-runner.sh wrapper

**Files:**
- Create: `scripts/docker-test-runner.sh`

This script runs INSIDE the Docker container. It:
1. Starts a Zellij session headlessly using `script` for PTY
2. Waits for the session to be ready
3. Discovers the pane ID
4. Runs the integration tests
5. Kills the session and exits with the test result

**Step 1: Write the wrapper script**

```bash
#!/usr/bin/env bash
#
# Docker test runner for zellij-tab-status integration tests.
# Runs INSIDE the Docker container. Starts Zellij headlessly,
# discovers pane ID, runs integration tests, cleans up.
#
set -euo pipefail

PLUGIN_WASM="/test/plugin.wasm"
SESSION="integration-test"

# --- Verify plugin exists ---
if [[ ! -f "$PLUGIN_WASM" ]]; then
    echo "ERROR: Plugin not found at $PLUGIN_WASM"
    echo "Mount it with: -v path/to/plugin.wasm:/test/plugin.wasm:ro"
    exit 1
fi

# --- Start Zellij headlessly ---
# `script` provides the pseudo-TTY that Zellij requires.
# We grant all permissions to avoid interactive prompts.
echo "Starting Zellij session '$SESSION'..."
script -qfc "zellij --session $SESSION options --disable-mouse-mode" /dev/null &
ZELLIJ_PID=$!

# Wait for session to be ready
for i in $(seq 1 30); do
    if zellij list-sessions 2>/dev/null | grep -q "$SESSION"; then
        echo "Zellij session ready (attempt $i)"
        break
    fi
    if ! kill -0 $ZELLIJ_PID 2>/dev/null; then
        echo "ERROR: Zellij process died"
        exit 1
    fi
    sleep 0.5
done

if ! zellij list-sessions 2>/dev/null | grep -q "$SESSION"; then
    echo "ERROR: Zellij session did not start within 15s"
    kill $ZELLIJ_PID 2>/dev/null || true
    exit 1
fi

# --- Discover pane ID ---
# Send a get_version command to the plugin — this also forces plugin load.
# Then query pane info to find our terminal pane ID.
export ZELLIJ_SESSION="$SESSION"

# Use zellij action list-clients or dump-layout to discover pane IDs.
# The simplest: use `zellij action dump-layout` and parse pane_id from it.
sleep 2  # let Zellij fully initialize and deliver TabUpdate/PaneUpdate

PANE_ID=$(zellij action dump-layout 2>/dev/null \
    | grep -oP 'terminal_pane_id="\K[0-9]+' \
    | head -1)

if [[ -z "$PANE_ID" ]]; then
    # Fallback: try numeric IDs from dump-layout
    PANE_ID=$(zellij action dump-layout 2>/dev/null \
        | grep -oP 'pane_id="\K[0-9]+' \
        | head -1)
fi

if [[ -z "$PANE_ID" ]]; then
    echo "ERROR: Could not discover pane ID from dump-layout"
    echo "Layout dump:"
    zellij action dump-layout 2>/dev/null || echo "(dump failed)"
    kill $ZELLIJ_PID 2>/dev/null || true
    exit 1
fi

echo "Discovered pane ID: $PANE_ID"

# --- Run integration tests ---
export ZELLIJ_PANE_ID="$PANE_ID"
export PLUGIN_PATH="file:$PLUGIN_WASM"

test_exit=0
/test/scripts/integration-test.sh || test_exit=$?

# --- Cleanup ---
zellij kill-session "$SESSION" 2>/dev/null || true
wait $ZELLIJ_PID 2>/dev/null || true

exit $test_exit
```

**Step 2: Make executable and test locally in Docker**

Run:
```bash
chmod +x scripts/docker-test-runner.sh
cargo build --release --target wasm32-wasip1
docker build -f Dockerfile.test -t zellij-tab-status-test .
docker run --rm \
    -v "$(pwd)/target/wasm32-wasip1/release/zellij-tab-status.wasm:/test/plugin.wasm:ro" \
    -v "$(pwd)/scripts:/test/scripts:ro" \
    zellij-tab-status-test \
    /test/scripts/docker-test-runner.sh
```

Expected: Zellij starts, pane ID is discovered, tests run. Some tests may fail (that's OK — we need to fix integration-test.sh next).

**Step 3: Commit**

```bash
git add scripts/docker-test-runner.sh
git commit -m "feat: add docker-test-runner.sh for headless integration tests"
```

---

### Task 3: Update integration-test.sh for Docker compatibility

**Files:**
- Modify: `scripts/integration-test.sh`

Changes needed:
1. Use `$PLUGIN_PATH` env var instead of hardcoded path (set by docker-test-runner.sh, or default to installed path for local use)
2. Keep `$ZELLIJ_PANE_ID` from env (already works — set by docker-test-runner.sh)

**Step 1: Update PLUGIN_PATH to use env var with fallback**

Replace line 13:
```bash
PLUGIN_PATH="file:$HOME/.config/zellij/plugins/zellij-tab-status.wasm"
```
With:
```bash
PLUGIN_PATH="${PLUGIN_PATH:-file:$HOME/.config/zellij/plugins/zellij-tab-status.wasm}"
```

This allows docker-test-runner.sh to set `PLUGIN_PATH=file:/test/plugin.wasm` while keeping backward compatibility for local runs.

**Step 2: Run tests in Docker to verify**

Run:
```bash
docker run --rm \
    -v "$(pwd)/target/wasm32-wasip1/release/zellij-tab-status.wasm:/test/plugin.wasm:ro" \
    -v "$(pwd)/scripts:/test/scripts:ro" \
    zellij-tab-status-test \
    /test/scripts/docker-test-runner.sh
```

Expected: Tests run, results printed, exit code reflects pass/fail count.

**Step 3: Commit**

```bash
git add scripts/integration-test.sh
git commit -m "fix: make integration-test.sh plugin path configurable via env"
```

---

### Task 4: Update Makefile

**Files:**
- Modify: `Makefile`

**Step 1: Replace test-integration target**

Replace:
```makefile
# Integration tests (must run inside Zellij session after make install)
test-integration:
	./scripts/integration-test.sh
```

With:
```makefile
# Integration tests (runs in Docker with real Zellij session)
test-integration: build
	docker build -f Dockerfile.test -t zellij-tab-status-test .
	docker run --rm \
		-v "$$(pwd)/$(TARGET):/test/plugin.wasm:ro" \
		-v "$$(pwd)/scripts:/test/scripts:ro" \
		zellij-tab-status-test \
		/test/scripts/docker-test-runner.sh
```

**Step 2: Verify locally**

Run: `make test-integration`
Expected: Builds .wasm, builds Docker image, runs tests inside container.

**Step 3: Commit**

```bash
git add Makefile
git commit -m "feat: make test-integration runs in Docker container"
```

---

### Task 5: Add integration tests to GitHub Actions CI

**Files:**
- Modify: `.github/workflows/ci.yml`

**Step 1: Add integration-test job**

Add after the existing `build` job:

```yaml
  integration-test:
    runs-on: ubuntu-latest
    needs: build
    steps:
      - uses: actions/checkout@v4

      - name: Download WASM artifact
        uses: actions/download-artifact@v4
        with:
          name: zellij-tab-status.wasm
          path: target/wasm32-wasip1/release/

      - name: Build test Docker image
        run: docker build -f Dockerfile.test -t zellij-tab-status-test .

      - name: Run integration tests
        run: |
          docker run --rm \
            -v "$(pwd)/target/wasm32-wasip1/release/zellij-tab-status.wasm:/test/plugin.wasm:ro" \
            -v "$(pwd)/scripts:/test/scripts:ro" \
            zellij-tab-status-test \
            /test/scripts/docker-test-runner.sh
```

This reuses the .wasm artifact from the build job — no need to rebuild.

**Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add Docker-based integration tests to CI pipeline"
```

---

### Task 6: Test full pipeline end-to-end

**Step 1: Run locally**

Run: `make test-integration`
Expected: All 12 assertions pass (2 existing + 10 that were failing due to dual-instance).

**Step 2: Debug any failures**

Check Docker logs. If Zellij fails to start:
- Check `script` command availability
- Check `zellij list-sessions` output
- Check `zellij action dump-layout` output for pane ID discovery

If tests fail:
- Check if plugin loads on-demand via `--plugin` flag
- Check if TabUpdate/PaneUpdate events arrive before pipe commands
- May need to increase sleep times in docker-test-runner.sh

**Step 3: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "fix: integration test adjustments for Docker environment"
```
