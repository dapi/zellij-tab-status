.PHONY: build install install-scripts clean test test-live test-integration test-plugin-dedup test-issue5-regression

PLUGIN_NAME = zellij-tab-status
TARGET = target/wasm32-wasip1/release/zellij-tab-status.wasm
INSTALL_DIR = $(HOME)/.config/zellij/plugins
SCRIPTS_DIR = $(HOME)/.local/bin
TEST_PLUGIN_MODE ?= preloaded

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

# Run unit tests (library only, no WASM runtime needed)
test:
	cargo test --lib

# Test in live Zellij session (run after installing and restarting zellij)
test-live:
	zellij pipe --plugin "file:$(INSTALL_DIR)/$(PLUGIN_NAME).wasm" --name tab-status -- '{"pane_id": "'"$$ZELLIJ_PANE_ID"'", "action": "set_status", "emoji": "🧪"}' < /dev/null
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
