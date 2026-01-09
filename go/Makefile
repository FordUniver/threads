.PHONY: build install clean test fmt lint

BINARY := threads
PKG := git.zib.de/cspiegel/threads
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
LDFLAGS := -ldflags "-s -w -X main.version=$(VERSION)"

build:
	go build $(LDFLAGS) -trimpath -o $(BINARY) ./cmd/threads

install:
	go install $(LDFLAGS) ./cmd/threads

clean:
	rm -f $(BINARY)
	go clean

test:
	go test -v ./...

fmt:
	go fmt ./...

lint:
	golangci-lint run

# Development helpers
.PHONY: run deps

run: build
	./$(BINARY) $(ARGS)

deps:
	go mod download
	go mod tidy
