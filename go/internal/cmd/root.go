package cmd

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/workspace"
)

var ws string

var rootCmd = &cobra.Command{
	Use:   "threads",
	Short: "Thread management for LLM workflows",
	Long: `threads - Persistent context management for LLM-assisted development.

Threads are markdown files in .threads/ directories at workspace, category,
or project level. Each thread tracks a single topic: a feature, bug,
exploration, or decision.`,
	SilenceUsage:  true,
	SilenceErrors: true,
	PersistentPreRunE: func(cmd *cobra.Command, args []string) error {
		var err error
		ws, err = workspace.Find()
		if err != nil {
			return fmt.Errorf("workspace not found: %w", err)
		}
		return nil
	},
}

func Execute() error {
	return rootCmd.Execute()
}

func init() {
	// Workspace operations
	rootCmd.AddCommand(listCmd)
	rootCmd.AddCommand(newCmd)
	rootCmd.AddCommand(moveCmd)
	rootCmd.AddCommand(commitCmd)
	rootCmd.AddCommand(validateCmd)
	rootCmd.AddCommand(gitCmd)
	rootCmd.AddCommand(statsCmd)

	// Single-thread operations
	rootCmd.AddCommand(readCmd)
	rootCmd.AddCommand(statusCmd)
	rootCmd.AddCommand(updateCmd)
	rootCmd.AddCommand(bodyCmd)
	rootCmd.AddCommand(noteCmd)
	rootCmd.AddCommand(todoCmd)
	rootCmd.AddCommand(logCmd)
	rootCmd.AddCommand(resolveCmd)
	rootCmd.AddCommand(reopenCmd)
	rootCmd.AddCommand(removeCmd)
}

// getWorkspace returns the cached workspace path
func getWorkspace() string {
	return ws
}

// fatal prints an error and exits
func fatal(format string, args ...interface{}) {
	fmt.Fprintf(os.Stderr, format+"\n", args...)
	os.Exit(1)
}
