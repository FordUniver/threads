package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	moveCommit bool
	moveMsg    string
)

var moveCmd = &cobra.Command{
	Use:   "move <id> <new-path>",
	Short: "Move thread to new location",
	Long: `Move a thread to a new location.

Path resolution for new-path:
  .       → PWD (explicit)
  ./X/Y   → PWD-relative
  /X/Y    → Absolute
  X/Y     → Git-root-relative`,
	Args:              cobra.ExactArgs(2),
	ValidArgsFunction: completeThreadIDs,
	RunE:              runMove,
}

func init() {
	moveCmd.Flags().BoolVar(&moveCommit, "commit", false, "Commit after moving")
	moveCmd.Flags().StringVarP(&moveMsg, "m", "m", "", "Commit message")
}

func runMove(cmd *cobra.Command, args []string) error {
	gitRoot := getWorkspace()
	ref := args[0]
	newPath := args[1]

	// Find source thread
	srcFile, err := workspace.FindByRef(gitRoot, ref)
	if err != nil {
		return err
	}

	// Resolve destination scope
	scope, err := workspace.InferScope(gitRoot, newPath)
	if err != nil {
		return fmt.Errorf("invalid path '%s': %v", newPath, err)
	}

	// Ensure dest .threads/ exists
	if err := os.MkdirAll(scope.ThreadsDir, 0755); err != nil {
		return fmt.Errorf("creating threads directory: %w", err)
	}

	// Move file
	filename := filepath.Base(srcFile)
	destFile := filepath.Join(scope.ThreadsDir, filename)

	if _, err := os.Stat(destFile); err == nil {
		return fmt.Errorf("thread already exists at destination: %s", destFile)
	}

	if err := os.Rename(srcFile, destFile); err != nil {
		return fmt.Errorf("moving file: %w", err)
	}

	relDest := workspace.PathRelativeToGitRoot(gitRoot, destFile)
	fmt.Printf("Moved to %s\n", scope.LevelDesc)
	fmt.Printf("  → %s\n", relDest)

	// Commit if requested
	if moveCommit {
		relSrc := workspace.PathRelativeToGitRoot(gitRoot, srcFile)
		if err := git.Add(gitRoot, relSrc, relDest); err != nil {
			return err
		}
		msg := moveMsg
		if msg == "" {
			msg = fmt.Sprintf("threads: move %s to %s", filepath.Base(srcFile), scope.LevelDesc)
		}
		if err := git.Commit(gitRoot, []string{relSrc, relDest}, msg); err != nil {
			return err
		}
		fmt.Println("Note: Changes are local. Push with 'git push' when ready.")
	} else {
		fmt.Println("Note: Use --commit to commit this move")
	}

	return nil
}
