package cmd

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var gitCmd = &cobra.Command{
	Use:   "git",
	Short: "Show pending thread changes",
	RunE:  runGit,
}

func runGit(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()

	threads, err := workspace.FindAllThreads(ws)
	if err != nil {
		return err
	}

	var modified []string
	for _, t := range threads {
		relPath, _ := filepath.Rel(ws, t)
		if git.HasChanges(ws, relPath) {
			modified = append(modified, relPath)
		}
	}

	if len(modified) == 0 {
		fmt.Println("No pending thread changes.")
		return nil
	}

	fmt.Println("Pending thread changes:")
	for _, f := range modified {
		fmt.Printf("  %s\n", f)
	}
	fmt.Println()
	fmt.Println("Suggested:")
	fmt.Printf("  git add %s && git commit -m \"threads: update\" && git push\n", strings.Join(modified, " "))

	return nil
}
