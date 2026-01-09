package main

import (
	"os"

	"git.zib.de/cspiegel/threads/internal/cmd"
)

func main() {
	if err := cmd.Execute(); err != nil {
		os.Exit(1)
	}
}
