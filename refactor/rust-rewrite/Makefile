.PHONY: build test run-dev run-prod clean fmt clippy check install

# Build all crates
build:
	cargo build

# Build in release mode
build-release:
	cargo build --release

# Run all tests
test:
	cargo test

# Run tests with verbose output
test-verbose:
	cargo test -- --nocapture

# Run integration tests only
test-integration:
	cargo test --test integration_test

# Run development server
run-dev:
	cargo run -- dev --verbose

# Run development server with custom port
run-dev-port:
	cargo run -- dev --port 3000 --verbose

# Run production server
run-prod:
	cargo run -- start

# Clean build artifacts
clean:
	cargo clean

# Format code
fmt:
	cargo fmt

# Run clippy linter
clippy:
	cargo clippy -- -D warnings

# Check code without building
check:
	cargo check

# Install development dependencies
install:
	cargo install cargo-watch
	cargo install cargo-audit
	cargo install cargo-outdated

# Watch for changes and rebuild
watch:
	cargo watch -x check -x test

# Security audit
audit:
	cargo audit

# Check for outdated dependencies
outdated:
	cargo outdated

# Run all quality checks
quality: fmt clippy test audit

# Docker build
docker-build:
	docker build -t inngest-rust .

# Docker run dev server
docker-run-dev:
	docker run -p 8288:8288 inngest-rust dev

# Benchmark
bench:
	cargo bench

# Generate documentation
docs:
	cargo doc --open

# Update dependencies
update:
	cargo update

# Profile the application
profile-dev:
	cargo run --release -- dev &
	sleep 5
	curl http://localhost:8288/health
	killall inngest
