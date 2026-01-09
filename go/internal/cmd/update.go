package cmd

import (
	"fmt"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	updateTitle  string
	updateDesc   string
	updateCommit bool
	updateMsg    string
)

var updateCmd = &cobra.Command{
	Use:   "update <id>",
	Short: "Update thread title/desc",
	Args:  cobra.ExactArgs(1),
	RunE:  runUpdate,
}

func init() {
	updateCmd.Flags().StringVar(&updateTitle, "title", "", "New title")
	updateCmd.Flags().StringVar(&updateDesc, "desc", "", "New description")
	updateCmd.Flags().BoolVar(&updateCommit, "commit", false, "Commit after updating")
	updateCmd.Flags().StringVarP(&updateMsg, "m", "m", "", "Commit message")
}

func runUpdate(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	ref := args[0]

	if updateTitle == "" && updateDesc == "" {
		return fmt.Errorf("specify --title and/or --desc")
	}

	file, err := workspace.FindByRef(ws, ref)
	if err != nil {
		return err
	}

	t, err := thread.Parse(file)
	if err != nil {
		return err
	}

	if updateTitle != "" {
		if err := t.SetFrontmatterField("name", updateTitle); err != nil {
			return err
		}
		fmt.Printf("Title updated: %s\n", updateTitle)
	}

	if updateDesc != "" {
		if err := t.SetFrontmatterField("desc", updateDesc); err != nil {
			return err
		}
		fmt.Printf("Description updated: %s\n", updateDesc)
	}

	if err := t.Write(); err != nil {
		return err
	}

	fmt.Printf("Updated: %s\n", file)

	if updateCommit {
		msg := updateMsg
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
