package main

import (
	"fmt"
	"os"

	"git.zib.de/cspiegel/threads/internal/cmd"
)

// version is set via ldflags: -X main.version=$(git describe --tags --always --dirty)
var version = "dev"

func main() {
	cmd.SetVersion(version)
	if err := cmd.Execute(); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}
