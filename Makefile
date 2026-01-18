# threads CLI - Multi-language comparison study
# Run tests and benchmarks across all implementations

.PHONY: help test test-go test-python test-perl test-rust test-swift test-ruby test-bun test-all test-validate benchmark benchmark-quick benchmark-full clean

# Default target
help:
	@echo "threads CLI test and benchmark targets"
	@echo ""
	@echo "Testing:"
	@echo "  make test          - Test Rust implementation (default)"
	@echo "  make test-go       - Test Go implementation"
	@echo "  make test-python   - Test Python implementation"
	@echo "  make test-perl     - Test Perl implementation"
	@echo "  make test-rust     - Test Rust implementation"
	@echo "  make test-swift    - Test Swift implementation"
	@echo "  make test-ruby     - Test Ruby implementation"
	@echo "  make test-bun      - Test Bun/TypeScript implementation"
	@echo "  make test-all      - Test ALL 7 implementations"
	@echo "  make test-validate - Verify tests pass individually (isolation check)"
	@echo ""
	@echo "Benchmarking:"
	@echo "  make benchmark       - Run default benchmarks"
	@echo "  make benchmark-quick - Quick benchmarks (~5 min)"
	@echo "  make benchmark-full  - Full benchmark suite (~30 min)"
	@echo ""
	@echo "Building:"
	@echo "  make build-go      - Build Go implementation"
	@echo "  make build-rust    - Build Rust implementation"
	@echo "  make build-swift   - Build Swift implementation"
	@echo "  make build-all     - Build all compiled implementations"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean         - Clean build artifacts"

# Test rust (default)
test: test-rust

test-go: build-go
	@echo "=== Testing Go ==="
	./test/run_tests.sh ./go/threads

test-python:
	@echo "=== Testing Python ==="
	./test/run_tests.sh ./python/threads-wrapper

test-perl:
	@echo "=== Testing Perl ==="
	./test/run_tests.sh "perl -I./perl/lib ./perl/bin/threads"

test-rust: build-rust
	@echo "=== Testing Rust ==="
	./test/run_tests.sh ./rust/target/release/threads

test-swift: build-swift
	@echo "=== Testing Swift ==="
	./test/run_tests.sh ./swift/.build/release/threads

test-ruby:
	@echo "=== Testing Ruby ==="
	./test/run_tests.sh ./ruby/bin/threads

test-bun:
	@echo "=== Testing Bun ==="
	./test/run_tests.sh ./bun/bin/threads

# Test all implementations
test-all: build-all
	@echo ""
	@echo "=========================================="
	@echo "Testing ALL 7 implementations"
	@echo "=========================================="
	@echo ""
	@echo "=== Go ===" && ./test/run_tests.sh ./go/threads && \
	echo "" && \
	echo "=== Python ===" && ./test/run_tests.sh "uv run --quiet --directory ./python python -m threads" && \
	echo "" && \
	echo "=== Perl ===" && ./test/run_tests.sh "perl -I./perl/lib ./perl/bin/threads" && \
	echo "" && \
	echo "=== Rust ===" && ./test/run_tests.sh ./rust/target/release/threads && \
	echo "" && \
	echo "=== Swift ===" && ./test/run_tests.sh ./swift/.build/release/threads && \
	echo "" && \
	echo "=== Ruby ===" && ./test/run_tests.sh ./ruby/bin/threads && \
	echo "" && \
	echo "=== Bun ===" && ./test/run_tests.sh ./bun/bin/threads && \
	echo "" && \
	echo "========================================" && \
	echo "All 7 implementations passed!" && \
	echo "========================================"

# Validate test isolation
test-validate: build-rust
	@echo "Validating test isolation for rust..."
	./test/run_tests.sh --validate ./rust/target/release/threads

test-validate-all: build-all
	@echo "Validating test isolation for ALL implementations..."
	@echo ""
	@echo "=== Go ===" && ./test/run_tests.sh --validate ./go/threads && \
	echo "" && \
	echo "=== Python ===" && ./test/run_tests.sh --validate "uv run --quiet --directory ./python python -m threads" && \
	echo "" && \
	echo "=== Perl ===" && ./test/run_tests.sh --validate "perl -I./perl/lib ./perl/bin/threads" && \
	echo "" && \
	echo "=== Rust ===" && ./test/run_tests.sh --validate ./rust/target/release/threads && \
	echo "" && \
	echo "=== Swift ===" && ./test/run_tests.sh --validate ./swift/.build/release/threads && \
	echo "" && \
	echo "=== Ruby ===" && ./test/run_tests.sh --validate ./ruby/bin/threads && \
	echo "" && \
	echo "=== Bun ===" && ./test/run_tests.sh --validate ./bun/bin/threads

# Benchmark all implementations
benchmark: build-all
	./test/benchmark.sh

benchmark-quick: build-all
	./test/benchmark.sh --quick

benchmark-full: build-all
	./test/benchmark.sh --full

# Build targets - check ALL source files, not just one
build-go:
	@newest=$$(find ./go -name '*.go' -newer ./go/threads 2>/dev/null | head -1); \
	if [ ! -f ./go/threads ] || [ -n "$$newest" ]; then \
		echo "Building Go implementation..."; \
		$(MAKE) -C go build; \
	fi

build-rust:
	@newest=$$(find ./rust/src -name '*.rs' -newer ./rust/target/release/threads 2>/dev/null | head -1); \
	if [ ! -f ./rust/target/release/threads ] || [ -n "$$newest" ]; then \
		echo "Building Rust implementation..."; \
		cd rust && cargo build --release; \
	fi

build-swift:
	@newest=$$(find ./swift/Sources -name '*.swift' -newer ./swift/.build/release/threads 2>/dev/null | head -1); \
	if [ ! -f ./swift/.build/release/threads ] || [ -n "$$newest" ]; then \
		echo "Building Swift implementation..."; \
		cd swift && swift build -c release; \
	fi

build-all: build-go build-rust build-swift
	@echo "All compiled implementations built"

# Clean
clean:
	$(MAKE) -C go clean 2>/dev/null || true
	cd rust && cargo clean 2>/dev/null || true
	rm -rf ./swift/.build 2>/dev/null || true
	rm -rf ./python/.venv 2>/dev/null || true
	rm -rf ./test/results/* 2>/dev/null || true
	find . -name "*.pyc" -delete 2>/dev/null || true
	find . -name "__pycache__" -type d -delete 2>/dev/null || true
