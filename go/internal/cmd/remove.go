package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	removeCommit bool
	removeMsg    string
)

var removeCmd = &cobra.Command{
	Use:               "remove <id>",
	Aliases:           []string{"rm"},
	Short:             "Remove thread entirely",
	Args:              cobra.ExactArgs(1),
	ValidArgsFunction: completeThreadIDs,
	RunE:              runRemove,
}

func init() {
	removeCmd.Flags().BoolVar(&removeCommit, "commit", false, "Commit after removing")
	removeCmd.Flags().StringVarP(&removeMsg, "m", "m", "", "Commit message")
}

func runRemove(cmd *cobra.Command, args []string) error {
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

	name := t.Name()
	relPath, _ := filepath.Rel(ws, file)

	// Check if file is tracked
	wasTracked := git.IsTracked(ws, relPath)

	// Remove file
	if err := os.Remove(file); err != nil {
		return fmt.Errorf("removing file: %w", err)
	}

	fmt.Printf("Removed: %s\n", file)

	if !wasTracked {
		fmt.Println("Note: Thread was never committed to git, no commit needed.")
		return nil
	}

	if removeCommit {
		msg := removeMsg
		if msg == "" {
			msg = fmt.Sprintf("threads: remove '%s'", name)
		}
		if err := git.Add(ws, relPath); err != nil {
			return err
		}
		if err := git.Commit(ws, []string{relPath}, msg); err != nil {
			return err
		}
		if err := git.Push(ws); err != nil {
			fmt.Printf("WARNING: git push failed (commit succeeded): %v\n", err)
		}
	} else {
		fmt.Println("Note: To commit this removal, run:")
		fmt.Printf("  git -C \"$WORKSPACE\" add \"%s\" && git -C \"$WORKSPACE\" commit -m \"threads: remove '%s'\"\n", relPath, name)
	}

	return nil
}
