.PHONY: build install install-scripts clean test test-live test-integration test-plugin-dedup test-issue5-regression test-issue6-regression test-issue6-raw-regression test-issue10-regression

PLUGIN_NAME = zellij-tab-status
TARGET = target/wasm32-wasip1/release/zellij-tab-status.wasm
INSTALL_DIR = $(HOME)/.config/zellij/plugins
SCRIPTS_DIR = $(HOME)/.local/bin
TEST_PLUGIN_MODE ?= preloaded
ISSUE10_ATTEMPTS ?= 3
ISSUE10_TAB_COUNT ?= 24
ISSUE10_PROBE_ROUNDS ?= 10
ISSUE10_READY_TIMEOUT ?= 20
ISSUE10_ACTION_TIMEOUT ?= 5
ISSUE10_RUN_TIMEOUT ?= 240

build:
	cargo build --release --target wasm32-wasip1

install: build install-scripts
	mkdir -p $(INSTALL_DIR)
	cp $(TARGET) $(INSTALL_DIR)/$(PLUGIN_NAME).wasm
	@echo ""
	@echo "✅ Installed:"
	@echo "   • Plugin: $(INSTALL_DIR)/$(PLUGIN_NAME).wasm"
	@echo "   • Script: $(SCRIPTS_DIR)/zellij-tab-status"
	@echo ""
	@echo "🔄 Restart zellij session to load the plugin."

install-scripts:
	mkdir -p $(SCRIPTS_DIR)
	cp scripts/zellij-tab-status $(SCRIPTS_DIR)/
	@echo "Installed scripts to $(SCRIPTS_DIR)/"

clean:
	cargo clean

# Run host tests (bin test harness is disabled in Cargo.toml)
test:
	cargo test

# Test in live Zellij session (run after installing and restarting zellij)
test-live:
	zellij pipe --plugin "file:$(INSTALL_DIR)/$(PLUGIN_NAME).wasm" -- '{"pane_id": "'"$$ZELLIJ_PANE_ID"'", "action": "set_status", "emoji": "🧪"}' < /dev/null
	sleep 0.5
	zellij action query-tab-names

# Integration tests (runs in Docker with real Zellij session)
test-integration: build
	docker build -f Dockerfile.test -t zellij-tab-status-test .
	docker run --rm \
		-e TEST_PLUGIN_MODE="$(TEST_PLUGIN_MODE)" \
		-v "$$(pwd)/$(TARGET):/test/plugin.wasm:ro" \
		-v "$$(pwd)/scripts:/test/scripts:ro" \
		zellij-tab-status-test \
		/test/scripts/docker-test-runner.sh

# On-demand mode smoke test: repeated --plugin calls should not duplicate instance
test-plugin-dedup: build
	docker build -f Dockerfile.test -t zellij-tab-status-test .
	docker run --rm \
		-v "$$(pwd)/$(TARGET):/test/plugin.wasm:ro" \
		-v "$$(pwd)/scripts:/test/scripts:ro" \
		zellij-tab-status-test \
		/test/scripts/docker-plugin-dedup-check.sh

# Regression check for issue #5 (on-demand --plugin + floating panel input)
test-issue5-regression: build
	docker build -f Dockerfile.test -t zellij-tab-status-test .
	docker run --rm \
		-v "$$(pwd)/$(TARGET):/test/plugin.wasm:ro" \
		-v "$$(pwd)/scripts:/test/scripts:ro" \
		zellij-tab-status-test \
		/test/scripts/issue5-regression-check.sh

# Regression check for issue #6 (first set_status ignored in fresh session)
test-issue6-regression: build
	docker build -f Dockerfile.test -t zellij-tab-status-test .
	docker run --rm \
		-v "$$(pwd)/$(TARGET):/test/plugin.wasm:ro" \
		-v "$$(pwd)/scripts:/test/scripts:ro" \
		zellij-tab-status-test \
		/test/scripts/issue6-regression-check.sh

# Regression check for issue #6 (single raw --plugin call must apply)
test-issue6-raw-regression: build
	docker build -f Dockerfile.test -t zellij-tab-status-test .
	docker run --rm \
		-v "$$(pwd)/$(TARGET):/test/plugin.wasm:ro" \
		-v "$$(pwd)/scripts:/test/scripts:ro" \
		zellij-tab-status-test \
		/test/scripts/issue6-raw-regression-check.sh

# Regression check for issue #10 (many tabs: no lost names after probing)
test-issue10-regression: build
	docker build -f Dockerfile.test -t zellij-tab-status-test .
	docker run --rm \
		-e ISSUE10_ATTEMPTS="$(ISSUE10_ATTEMPTS)" \
		-e ISSUE10_TAB_COUNT="$(ISSUE10_TAB_COUNT)" \
		-e ISSUE10_PROBE_ROUNDS="$(ISSUE10_PROBE_ROUNDS)" \
		-e ISSUE10_READY_TIMEOUT="$(ISSUE10_READY_TIMEOUT)" \
		-e ISSUE10_ACTION_TIMEOUT="$(ISSUE10_ACTION_TIMEOUT)" \
		-e ISSUE10_RUN_TIMEOUT="$(ISSUE10_RUN_TIMEOUT)" \
		-v "$$(pwd)/$(TARGET):/test/plugin.wasm:ro" \
		-v "$$(pwd)/scripts:/test/scripts:ro" \
		zellij-tab-status-test \
		/test/scripts/issue10-regression-check.sh
