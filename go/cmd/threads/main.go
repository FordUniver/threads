package main

import (
	"fmt"
	"os"

	"git.zib.de/cspiegel/threads/internal/cmd"
)

func main() {
	if err := cmd.Execute(); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}
