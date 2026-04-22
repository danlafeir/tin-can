APP_NAME  = tin-can
BUILD_DIR = bin

OS        = $(shell uname -s | tr '[:upper:]' '[:lower:]')
ARCH      = $(shell uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')
GIT_HASH  = $(shell git rev-parse --short HEAD)

BINARY_NAME = $(APP_NAME)-$(OS)-$(ARCH)-$(GIT_HASH)

.PHONY: all build build-all build-voice install clean test run deploy

# ── Local build ───────────────────────────────────────────────────────────────

all: build

build:
	@echo "Building $(APP_NAME) for $(OS)/$(ARCH)..."
	@mkdir -p $(BUILD_DIR)
	cargo build --release --features voice
	cp target/release/$(APP_NAME) $(BUILD_DIR)/$(APP_NAME)
	cp target/release/$(APP_NAME) $(BUILD_DIR)/$(BINARY_NAME)
	@echo "Built: $(BUILD_DIR)/$(BINARY_NAME)"

# Voice requires opus — macOS only, build locally
build-voice:
	@echo "Building $(APP_NAME) with voice for $(OS)/$(ARCH)..."
	@mkdir -p $(BUILD_DIR)
	cargo build --release --features voice
	cp target/release/$(APP_NAME) $(BUILD_DIR)/$(APP_NAME)-voice-$(OS)-$(ARCH)-$(GIT_HASH)
	@echo "Built: $(BUILD_DIR)/$(APP_NAME)-voice-$(OS)-$(ARCH)-$(GIT_HASH)"

# ── Cross-platform release build ─────────────────────────────────────────────
# Prerequisites:
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
#   cargo install cross   (requires Docker for Linux targets)

build-all:
	@echo "Building $(APP_NAME) for all supported platforms..."
	@rm -rf $(BUILD_DIR)
	@mkdir -p $(BUILD_DIR)

	@# Current platform (convenience copy for local use)
	cargo build --release --features voice
	cp target/release/$(APP_NAME) $(BUILD_DIR)/$(APP_NAME)

	@# macOS ARM64
	rustup target add aarch64-apple-darwin 2>/dev/null; \
	cargo build --release --features voice --target aarch64-apple-darwin
	cp target/aarch64-apple-darwin/release/$(APP_NAME) $(BUILD_DIR)/$(APP_NAME)-darwin-arm64-$(GIT_HASH)

	@# macOS AMD64
	rustup target add x86_64-apple-darwin 2>/dev/null; \
	cargo build --release --features voice --target x86_64-apple-darwin
	cp target/x86_64-apple-darwin/release/$(APP_NAME) $(BUILD_DIR)/$(APP_NAME)-darwin-amd64-$(GIT_HASH)

	@# Linux — static musl binaries via cross (needs Docker)
	@# Install with: cargo install cross
	@if command -v cross >/dev/null 2>&1; then \
		cross build --release --features voice --target x86_64-unknown-linux-musl && \
		cp target/x86_64-unknown-linux-musl/release/$(APP_NAME) $(BUILD_DIR)/$(APP_NAME)-linux-amd64-$(GIT_HASH) && \
		cross build --release --features voice --target aarch64-unknown-linux-musl && \
		cp target/aarch64-unknown-linux-musl/release/$(APP_NAME) $(BUILD_DIR)/$(APP_NAME)-linux-arm64-$(GIT_HASH); \
	else \
		echo "  [skip] cross not found — Linux binaries not built (cargo install cross)"; \
	fi

	@echo "Done:"
	@ls -lh $(BUILD_DIR)

# ── Utility ───────────────────────────────────────────────────────────────────

install: build
	@mkdir -p ~/.local/bin
	cp $(BUILD_DIR)/$(APP_NAME) ~/.local/bin/$(APP_NAME)
	@echo "Installed to ~/.local/bin/$(APP_NAME)"

clean:
	rm -rf $(BUILD_DIR)

test:
	cargo test

run:
	cargo run -- $(ARGS)

# ── Deploy ────────────────────────────────────────────────────────────────────
# Runs tests, builds all platforms, commits binaries, and pushes.

deploy: test build-all
	git add $(BUILD_DIR)/
	git commit -m "Pushing new tin-can build"
	git push
