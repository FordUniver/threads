# threads CLI
# Build, test, and benchmark

.PHONY: help build unit-test integration-test test benchmark clean

# Default target
help:
	@echo "threads CLI"
	@echo ""
	@echo "Building:"
	@echo "  make build            - Build threads binary"
	@echo ""
	@echo "Testing:"
	@echo "  make test             - Run all tests (unit + integration)"
	@echo "  make unit-test        - Run Go unit tests"
	@echo "  make integration-test - Run integration tests"
	@echo ""
	@echo "Benchmarking:"
	@echo "  make benchmark        - Run benchmarks"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean            - Clean build artifacts"

# Build
build:
	@if [ ! -f ./go/threads ] || [ -n "$$(find ./go -name '*.go' -newer ./go/threads 2>/dev/null | head -1)" ]; then \
		echo "Building threads..."; \
		$(MAKE) -C go build; \
	fi

# Test targets
test: unit-test integration-test

unit-test:
	@echo "=== Unit Tests ==="
	cd go && go test ./...

integration-test: build
	@echo "=== Integration Tests ==="
	./test/run_tests.sh ./go/threads

# Benchmark
benchmark: build
	@echo "=== Benchmark ==="
	./test/benchmark/bench.sh

# Clean
clean:
	$(MAKE) -C go clean 2>/dev/null || true
	rm -rf ./test/results/* 2>/dev/null || true
