# threads CLI (Rust)
# Build, test, and benchmark

.PHONY: help build test integration-test benchmark clean release

# Default target
help:
	@echo "threads CLI"
	@echo ""
	@echo "Building:"
	@echo "  make build            - Build debug binary"
	@echo "  make release          - Build optimized release binary"
	@echo ""
	@echo "Testing:"
	@echo "  make test             - Run all tests"
	@echo "  make integration-test - Run integration tests"
	@echo ""
	@echo "Benchmarking:"
	@echo "  make benchmark        - Run benchmarks"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean            - Clean build artifacts"

# Build
build:
	cargo build

release:
	cargo build --release

# Test targets
test:
	cargo test

integration-test: build
	@echo "=== Integration Tests ==="
	./test/run_tests.sh ./target/debug/threads

# Benchmark
benchmark: release
	@echo "=== Benchmark ==="
	./test/benchmark/bench.sh

# Clean
clean:
	cargo clean
	rm -rf ./test/results/* 2>/dev/null || true
