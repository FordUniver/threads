package cmd

import (
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	logCommit bool
	logMsg    string
)

var logCmd = &cobra.Command{
	Use:   "log <id> [entry]",
	Short: "Add log entry",
	Long:  `Add a timestamped entry to the Log section.`,
	Args:  cobra.RangeArgs(1, 2),
	RunE:  runLog,
}

func init() {
	logCmd.Flags().BoolVar(&logCommit, "commit", false, "Commit after adding")
	logCmd.Flags().StringVarP(&logMsg, "m", "m", "", "Commit message")
}

func runLog(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	ref := args[0]

	var entry string
	if len(args) >= 2 {
		entry = args[1]
	}

	// Read entry from stdin if not provided
	if entry == "" {
		stat, _ := os.Stdin.Stat()
		if (stat.Mode() & os.ModeCharDevice) == 0 {
			data, err := io.ReadAll(os.Stdin)
			if err != nil {
				return err
			}
			entry = string(data)
		}
	}

	if entry == "" {
		return fmt.Errorf("no log entry provided")
	}

	file, err := workspace.FindByRef(ws, ref)
	if err != nil {
		return err
	}

	t, err := thread.Parse(file)
	if err != nil {
		return err
	}

	t.Content = thread.InsertLogEntry(t.Content, entry)

	if err := t.Write(); err != nil {
		return err
	}

	fmt.Printf("Logged to: %s\n", file)

	if logCommit {
		msg := logMsg
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
