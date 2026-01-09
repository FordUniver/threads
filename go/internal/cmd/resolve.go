package cmd

import (
	"fmt"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	resolveCommit bool
	resolveMsg    string
)

var resolveCmd = &cobra.Command{
	Use:   "resolve <id>",
	Short: "Mark thread resolved",
	Args:  cobra.ExactArgs(1),
	RunE:  runResolve,
}

func init() {
	resolveCmd.Flags().BoolVar(&resolveCommit, "commit", false, "Commit after resolving")
	resolveCmd.Flags().StringVarP(&resolveMsg, "m", "m", "", "Commit message")
}

func runResolve(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	ref := args[0]

	file, err := workspace.FindByRef(ws, ref)
	if err != nil {
		return err
	}

	t, err := thread.Parse(file)
	if err != nil {
		return err
	}

	oldStatus := t.Status()

	// Update status
	if err := t.SetFrontmatterField("status", "resolved"); err != nil {
		return err
	}

	// Add log entry
	t.Content = thread.InsertLogEntry(t.Content, "Resolved.")

	if err := t.Write(); err != nil {
		return err
	}

	fmt.Printf("Resolved: %s → resolved (%s)\n", oldStatus, file)

	if resolveCommit {
		msg := resolveMsg
		if msg == "" {
			msg = git.GenerateCommitMessage(ws, []string{file})
		}
		if err := git.AutoCommit(ws, file, msg); err != nil {
			return err
		}
	} else {
		fmt.Printf("Note: Thread %s has uncommitted changes. Use 'threads commit %s' when ready.\n", ref, ref)
	}

	return nil
}
