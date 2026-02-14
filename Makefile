.PHONY: build install install-scripts configure-zellij clean test test-live

PLUGIN_NAME = zellij-tab-status
TARGET = target/wasm32-wasip1/release/zellij-tab-status.wasm
INSTALL_DIR = $(HOME)/.config/zellij/plugins
SCRIPTS_DIR = $(HOME)/.local/bin

build:
	cargo build --release --target wasm32-wasip1

install: build install-scripts configure-zellij
	mkdir -p $(INSTALL_DIR)
	cp $(TARGET) $(INSTALL_DIR)/$(PLUGIN_NAME).wasm
	@echo ""
	@echo "✅ Installed:"
	@echo "   • Plugin: $(INSTALL_DIR)/$(PLUGIN_NAME).wasm"
	@echo "   • Scripts: $(SCRIPTS_DIR)/zellij-tab-status, $(SCRIPTS_DIR)/zellij-rename-tab"
	@echo ""
	@echo "🔄 Restart zellij session to load the plugin."

configure-zellij:
	@./scripts/configure-zellij.sh || echo "⚠️  Auto-config failed. Add plugin to config.kdl manually."

install-scripts:
	mkdir -p $(SCRIPTS_DIR)
	cp scripts/zellij-tab-status scripts/zellij-rename-tab $(SCRIPTS_DIR)/
	@echo "Installed scripts to $(SCRIPTS_DIR)/"

clean:
	cargo clean

# Run unit tests (library only, no WASM runtime needed)
test:
	cargo test --lib

# Test in live Zellij session (run after installing and restarting zellij)
test-live:
	zellij pipe --name tab-rename -- '{"pane_id": "$(ZELLIJ_PANE_ID)", "name": "Test-Rename"}' < /dev/null
	sleep 0.5
	zellij action query-tab-names
