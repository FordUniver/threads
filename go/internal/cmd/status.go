package cmd

import (
	"fmt"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	statusCommit bool
	statusMsg    string
)

var statusCmd = &cobra.Command{
	Use:   "status <id> <new-status>",
	Short: "Change thread status",
	Args:  cobra.ExactArgs(2),
	RunE:  runStatus,
}

func init() {
	statusCmd.Flags().BoolVar(&statusCommit, "commit", false, "Commit after changing")
	statusCmd.Flags().StringVarP(&statusMsg, "m", "m", "", "Commit message")
}

func runStatus(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	ref := args[0]
	newStatus := args[1]

	file, err := workspace.FindByRef(ws, ref)
	if err != nil {
		return err
	}

	t, err := thread.Parse(file)
	if err != nil {
		return err
	}

	oldStatus := t.Status()

	if err := t.SetFrontmatterField("status", newStatus); err != nil {
		return err
	}

	if err := t.Write(); err != nil {
		return err
	}

	fmt.Printf("Status changed: %s → %s (%s)\n", oldStatus, newStatus, file)

	if statusCommit {
		msg := statusMsg
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
