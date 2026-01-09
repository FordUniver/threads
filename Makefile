# threads CLI - Multi-language comparison study
# Run tests and benchmarks across all implementations

.PHONY: help test test-shell test-go test-python test-perl test-all test-validate benchmark clean

# Default target
help:
	@echo "threads CLI test and benchmark targets"
	@echo ""
	@echo "Testing:"
	@echo "  make test          - Test shell implementation (default)"
	@echo "  make test-shell    - Test shell implementation"
	@echo "  make test-go       - Test Go implementation"
	@echo "  make test-python   - Test Python implementation"
	@echo "  make test-perl     - Test Perl implementation"
	@echo "  make test-all      - Test ALL implementations"
	@echo "  make test-validate - Verify tests pass individually (isolation check)"
	@echo ""
	@echo "Benchmarking:"
	@echo "  make benchmark     - Run benchmarks across all implementations"
	@echo ""
	@echo "Building:"
	@echo "  make build-go      - Build Go implementation"
	@echo "  make build-all     - Build all implementations"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean         - Clean build artifacts"

# Test shell (default)
test: test-shell

test-shell:
	@echo "=== Testing Shell ==="
	./test/run_tests.sh ./shell/threads

test-go: build-go
	@echo "=== Testing Go ==="
	./test/run_tests.sh ./go/threads

test-python:
	@echo "=== Testing Python ==="
	./test/run_tests.sh "uv run --quiet --directory ./python python -m threads"

test-perl:
	@echo "=== Testing Perl ==="
	./test/run_tests.sh "perl -I./perl/lib ./perl/bin/threads"

# Test all implementations
test-all: build-go
	@echo ""
	@echo "=========================================="
	@echo "Testing ALL implementations"
	@echo "=========================================="
	@echo ""
	@echo "=== Shell ===" && ./test/run_tests.sh ./shell/threads && \
	echo "" && \
	echo "=== Go ===" && ./test/run_tests.sh ./go/threads && \
	echo "" && \
	echo "=== Python ===" && ./test/run_tests.sh "uv run --quiet --directory ./python python -m threads" && \
	echo "" && \
	echo "=== Perl ===" && ./test/run_tests.sh "perl -I./perl/lib ./perl/bin/threads" && \
	echo "" && \
	echo "========================================" && \
	echo "All implementations passed!" && \
	echo "========================================"

# Validate test isolation
test-validate:
	@echo "Validating test isolation for shell..."
	./test/run_tests.sh --validate ./shell/threads

test-validate-all: build-go
	@echo "Validating test isolation for ALL implementations..."
	@echo ""
	@echo "=== Shell ===" && ./test/run_tests.sh --validate ./shell/threads && \
	echo "" && \
	echo "=== Go ===" && ./test/run_tests.sh --validate ./go/threads && \
	echo "" && \
	echo "=== Python ===" && ./test/run_tests.sh --validate "uv run --quiet --directory ./python python -m threads" && \
	echo "" && \
	echo "=== Perl ===" && ./test/run_tests.sh --validate "perl -I./perl/lib ./perl/bin/threads"

# Benchmark all implementations
benchmark: build-go
	./test/benchmark.sh

# Build targets
build-go:
	@if [ ! -f ./go/threads ] || [ ./go/threads -ot ./go/cmd/threads/main.go ]; then \
		echo "Building Go implementation..."; \
		$(MAKE) -C go build; \
	fi

build-all: build-go
	@echo "All implementations built"

# Clean
clean:
	$(MAKE) -C go clean 2>/dev/null || true
	rm -rf ./python/.venv 2>/dev/null || true
	find . -name "*.pyc" -delete 2>/dev/null || true
	find . -name "__pycache__" -type d -delete 2>/dev/null || true
