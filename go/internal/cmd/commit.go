package cmd

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	commitPending bool
	commitMsg     string
	commitAuto    bool
)

var commitCmd = &cobra.Command{
	Use:   "commit [ids...]",
	Short: "Commit thread changes",
	Long: `Commit specific threads or all pending thread changes.

Use --pending to commit all modified threads at once.`,
	RunE: runCommit,
}

func init() {
	commitCmd.Flags().BoolVar(&commitPending, "pending", false, "Commit all modified threads")
	commitCmd.Flags().StringVarP(&commitMsg, "m", "m", "", "Commit message")
	commitCmd.Flags().BoolVar(&commitAuto, "auto", false, "Auto-accept generated message")
}

func runCommit(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	var files []string

	if commitPending {
		// Collect all thread files with uncommitted changes
		threads, err := workspace.FindAllThreads(ws)
		if err != nil {
			return err
		}

		for _, t := range threads {
			relPath, _ := filepath.Rel(ws, t)
			if git.HasChanges(ws, relPath) {
				files = append(files, t)
			}
		}
	} else {
		// Resolve provided IDs to files
		if len(args) == 0 {
			return fmt.Errorf("provide thread IDs or use --pending")
		}

		for _, id := range args {
			file, err := workspace.FindByRef(ws, id)
			if err != nil {
				return err
			}
			relPath, _ := filepath.Rel(ws, file)
			if !git.HasChanges(ws, relPath) {
				fmt.Printf("No changes in thread: %s\n", id)
				continue
			}
			files = append(files, file)
		}
	}

	if len(files) == 0 {
		fmt.Println("No threads to commit.")
		return nil
	}

	// Generate commit message if not provided
	msg := commitMsg
	if msg == "" {
		msg = git.GenerateCommitMessage(ws, files)
		fmt.Printf("Generated message: %s\n", msg)

		if !commitAuto && isTerminal() {
			reader := bufio.NewReader(os.Stdin)
			fmt.Print("Proceed? [Y/n] ")
			response, _ := reader.ReadString('\n')
			response = strings.TrimSpace(strings.ToLower(response))
			if response == "n" || response == "no" {
				fmt.Println("Aborted.")
				return nil
			}
		}
	}

	// Stage and commit
	var relPaths []string
	for _, f := range files {
		relPath, _ := filepath.Rel(ws, f)
		relPaths = append(relPaths, relPath)
	}

	if err := git.Commit(ws, relPaths, msg); err != nil {
		return err
	}

	if err := git.Push(ws); err != nil {
		fmt.Printf("WARNING: git push failed (commit succeeded): %v\n", err)
	}

	fmt.Printf("Committed %d thread(s).\n", len(files))
	return nil
}

func isTerminal() bool {
	stat, _ := os.Stdin.Stat()
	return (stat.Mode() & os.ModeCharDevice) != 0
}
