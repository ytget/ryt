SHELL := bash

.DEFAULT_GOAL := help

.PHONY: help
help: ## Available commands
	@echo "Available commands:"
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n\n"} /^[a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[0;33m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)
	@echo ""

##@ Build

.PHONY: build
build: ## Build application
	cargo build --release

.PHONY: install
install: ## Install application locally
	cargo install --path .

.PHONY: check
check: ## Check code without building
	cargo check

.PHONY: clippy
clippy: ## Run clippy linter
	cargo clippy -- -D warnings

.PHONY: fmt
fmt: ## Format code
	cargo fmt

##@ Test

.PHONY: test
test: ## Run tests
	cargo test

.PHONY: test-e2e
test-e2e: ## Run end-to-end tests (requires YTDLP_E2E=1)
	YTDLP_E2E=1 cargo test --test e2e

.PHONY: test-e2e-url
test-e2e-url: ## Run e2e test with specific URL: make test-e2e-url URL="https://..."
	YTDLP_E2E=1 YTDLP_E2E_URL="$(URL)" cargo test --test e2e

.PHONY: coverage
coverage: ## Run code coverage analysis
	cargo tarpaulin --out Stdout --timeout 120

.PHONY: coverage-html
coverage-html: ## Generate HTML coverage report
	cargo tarpaulin --out Html --timeout 120

##@ Download

.PHONY: download
download: build ## Build and download video: make download URL="https://..."
	@if [ -z "$(URL)" ]; then \
		echo "Error: URL is required. Usage: make download URL=\"https://youtube.com/watch?v=...\""; \
		exit 1; \
	fi
	FMT_FLAG=$$( [ -n "$(FORMAT)" ] && echo "--format $(FORMAT)" ); \
	EXT_FLAG=$$( [ -n "$(EXT)" ] && echo "--ext $(EXT)" ); \
	OUT_FLAG=$$( [ -n "$(OUTPUT)" ] && echo "--output $(OUTPUT)" ); \
	BG_FLAG=$$( [ -n "$(BOTGUARD)" ] && echo "--botguard $(BOTGUARD)" ); \
	DBG_FLAG=$$( [ -n "$(DEBUG_BOTGUARD)" ] && echo "--debug-botguard" ); \
	CNAME_FLAG=$$( [ -n "$(CLIENT_NAME)" ] && echo "--client-name $(CLIENT_NAME)" ); \
	CVER_FLAG=$$( [ -n "$(CLIENT_VERSION)" ] && echo "--client-version $(CLIENT_VERSION)" ); \
	UA_FLAG=$$( [ -n "$(USER_AGENT)" ] && echo "--user-agent $(USER_AGENT)" ); \
	PRN_FLAG=$$( [ -n "$(PRINT_URL)" ] && echo "-g" ); \
	./target/release/ryt $$FMT_FLAG $$EXT_FLAG $$OUT_FLAG $$BG_FLAG $$DBG_FLAG $$CNAME_FLAG $$CVER_FLAG $$UA_FLAG $$PRN_FLAG "$(URL)"

.PHONY: dl
dl: ## Build and download video (alias for download)
	@make download URL="$(URL)"

##@ Aliases

.PHONY: b
b: ## Build application
	@make build

.PHONY: i
i: ## Install application locally
	@make install

.PHONY: t
t: ## Run tests
	@make test

.PHONY: c
c: ## Run clippy linter
	@make clippy

.PHONY: f
f: ## Format code
	@make fmt

.PHONY: e
e: ## Run end-to-end tests
	@make test-e2e

.PHONY: eu
eu: ## Run e2e test with specific URL
	@make test-e2e-url URL="$(URL)"

.PHONY: cov
cov: ## Run code coverage analysis
	@make coverage

