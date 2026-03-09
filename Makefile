.PHONY: build install clean test test-integration

BINARY_NAME = zellij-tab-status
INSTALL_DIR = $(HOME)/.local/bin

build:
	cargo build --release

install: build
	mkdir -p $(INSTALL_DIR)
	cp target/release/$(BINARY_NAME) $(INSTALL_DIR)/
	@echo "Installed: $(INSTALL_DIR)/$(BINARY_NAME)"

clean:
	cargo clean

test:
	cargo test --lib

test-integration: build
	docker build -f Dockerfile.test -t zellij-tab-status-test .
	docker run --rm \
		-v "$$(pwd)/target/release/$(BINARY_NAME):/test/zellij-tab-status:ro" \
		-v "$$(pwd)/scripts:/test/scripts:ro" \
		zellij-tab-status-test \
		/test/scripts/docker-test-runner.sh
