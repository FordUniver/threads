package cmd

import (
	"fmt"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	reopenStatus string
	reopenCommit bool
	reopenMsg    string
)

var reopenCmd = &cobra.Command{
	Use:               "reopen <id>",
	Short:             "Reopen resolved thread",
	Args:              cobra.ExactArgs(1),
	ValidArgsFunction: completeThreadIDs,
	RunE:              runReopen,
}

func init() {
	reopenCmd.Flags().StringVar(&reopenStatus, "status", "active", "Status to reopen to")
	reopenCmd.Flags().BoolVar(&reopenCommit, "commit", false, "Commit after reopening")
	reopenCmd.Flags().StringVarP(&reopenMsg, "m", "m", "", "Commit message")
}

func runReopen(cmd *cobra.Command, args []string) error {
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
	if err := t.SetFrontmatterField("status", reopenStatus); err != nil {
		return err
	}

	// Add log entry
	t.Content = thread.InsertLogEntry(t.Content, "Reopened.")

	if err := t.Write(); err != nil {
		return err
	}

	fmt.Printf("Reopened: %s → %s (%s)\n", oldStatus, reopenStatus, file)

	if reopenCommit {
		msg := reopenMsg
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
