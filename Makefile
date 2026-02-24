.PHONY: all build test clean docker deploy lint fmt ci-local help

# Default target
all: build

# Build all components
build:
	@echo "Building MEV Protocol..."
	@cd fast && make
	@cd core && cargo build --release
	@cd network && go build -o ../bin/mev-node ./cmd/mev-node
	@cd contracts && forge build

# Run tests
test:
	@echo "Running tests..."
	@cd fast && make test
	@cd core && cargo test
	@cd network && go test ./...
	@cd contracts && forge test

# Format checks (Rust)
fmt:
	@cd core && cargo fmt --all -- --check

# Lint checks (Rust + Go)
lint:
	@cd core && cargo clippy --all-targets --all-features -- -D warnings
	@cd network && go vet ./...

# Local CI parity
ci-local: build test lint

# Help
help:
	@echo "Available targets:"
	@echo "  make build      - Build all components"
	@echo "  make test       - Run tests across stacks"
	@echo "  make fmt        - Run format checks"
	@echo "  make lint       - Run lint checks"
	@echo "  make ci-local   - Run build + test + lint"
	@echo "  make clean      - Clean build artifacts"
	@echo "  make docker     - Build Docker images"

# Clean build artifacts
clean:
	@cd fast && make clean
	@cd core && cargo clean
	@cd network && rm -f ../bin/mev-node
	@cd contracts && forge clean

# Build Docker images
docker:
	docker-compose -f docker/docker-compose.yml build

# Run with Docker
docker-up:
	docker-compose -f docker/docker-compose.yml up -d

docker-down:
	docker-compose -f docker/docker-compose.yml down

# Deploy contracts
deploy-arb:
	./scripts/deploy.sh arbitrum

deploy-base:
	./scripts/deploy.sh base

deploy-all:
	./scripts/deploy.sh ethereum
	./scripts/deploy.sh arbitrum
	./scripts/deploy.sh base
