package cmd

import (
	"fmt"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	noteCommit bool
	noteMsg    string
)

var noteCmd = &cobra.Command{
	Use:   "note <id> <action> [text|hash] [new-text]",
	Short: "Manage notes",
	Long: `Manage notes in the Notes section.

Actions:
  add <text>           Add a new note
  edit <hash> <text>   Edit a note by hash
  remove <hash>        Remove a note by hash`,
	Args: cobra.MinimumNArgs(2),
	RunE: runNote,
}

func init() {
	noteCmd.Flags().BoolVar(&noteCommit, "commit", false, "Commit after editing")
	noteCmd.Flags().StringVarP(&noteMsg, "m", "m", "", "Commit message")
}

func runNote(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	ref := args[0]
	action := args[1]

	file, err := workspace.FindByRef(ws, ref)
	if err != nil {
		return err
	}

	t, err := thread.Parse(file)
	if err != nil {
		return err
	}

	var logEntry string

	switch action {
	case "add":
		if len(args) < 3 {
			return fmt.Errorf("usage: threads note <id> add \"text\"")
		}
		text := args[2]

		var hash string
		t.Content, hash = thread.AddNote(t.Content, text)

		// Add log entry
		logEntry = fmt.Sprintf("Added note: %s", text)
		t.Content = thread.InsertLogEntry(t.Content, logEntry)

		fmt.Printf("Added note: %s (id: %s)\n", text, hash)

	case "edit":
		if len(args) < 4 {
			return fmt.Errorf("usage: threads note <id> edit <hash> \"new text\"")
		}
		hash := args[2]
		newText := args[3]

		// Check for ambiguous hash
		count := thread.CountMatchingItems(t.Content, "Notes", hash)
		if count == 0 {
			return fmt.Errorf("no note with hash '%s' found", hash)
		}
		if count > 1 {
			return fmt.Errorf("ambiguous hash '%s' matches %d notes", hash, count)
		}

		var editErr error
		t.Content, editErr = thread.EditByHash(t.Content, "Notes", hash, newText)
		if editErr != nil {
			return editErr
		}

		logEntry = fmt.Sprintf("Edited note %s", hash)
		t.Content = thread.InsertLogEntry(t.Content, logEntry)

		fmt.Printf("Edited note %s\n", hash)

	case "remove":
		if len(args) < 3 {
			return fmt.Errorf("usage: threads note <id> remove <hash>")
		}
		hash := args[2]

		// Check for ambiguous hash
		count := thread.CountMatchingItems(t.Content, "Notes", hash)
		if count == 0 {
			return fmt.Errorf("no note with hash '%s' found", hash)
		}
		if count > 1 {
			return fmt.Errorf("ambiguous hash '%s' matches %d notes", hash, count)
		}

		var removeErr error
		t.Content, removeErr = thread.RemoveByHash(t.Content, "Notes", hash)
		if removeErr != nil {
			return removeErr
		}

		logEntry = fmt.Sprintf("Removed note %s", hash)
		t.Content = thread.InsertLogEntry(t.Content, logEntry)

		fmt.Printf("Removed note %s\n", hash)

	default:
		return fmt.Errorf("unknown action '%s'. Use: add, edit, remove", action)
	}

	if err := t.Write(); err != nil {
		return err
	}

	if noteCommit {
		msg := noteMsg
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
