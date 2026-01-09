package cmd

import (
	"fmt"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	todoCommit bool
	todoMsg    string
)

var todoCmd = &cobra.Command{
	Use:   "todo <id> <action> [item|hash]",
	Short: "Manage todo items",
	Long: `Manage todo items in the Todo section.

Actions:
  add <text>     Add a new todo item
  check <hash>   Mark item as checked
  uncheck <hash> Mark item as unchecked
  remove <hash>  Remove item`,
	Args: cobra.MinimumNArgs(2),
	RunE: runTodo,
}

func init() {
	todoCmd.Flags().BoolVar(&todoCommit, "commit", false, "Commit after editing")
	todoCmd.Flags().StringVarP(&todoMsg, "m", "m", "", "Commit message")
}

func runTodo(cmd *cobra.Command, args []string) error {
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

	switch action {
	case "add":
		if len(args) < 3 {
			return fmt.Errorf("usage: threads todo <id> add \"item text\"")
		}
		text := args[2]

		var hash string
		t.Content, hash = thread.AddTodoItem(t.Content, text)

		fmt.Printf("Added to Todo: %s (id: %s)\n", text, hash)

	case "check", "complete", "done":
		if len(args) < 3 {
			return fmt.Errorf("usage: threads todo <id> check <hash>")
		}
		hash := args[2]

		// Check for ambiguous hash
		count := thread.CountMatchingItems(t.Content, "Todo", hash)
		if count == 0 {
			return fmt.Errorf("no unchecked item with hash '%s' found", hash)
		}
		if count > 1 {
			return fmt.Errorf("ambiguous hash '%s' matches %d items", hash, count)
		}

		var checkErr error
		t.Content, checkErr = thread.SetTodoChecked(t.Content, hash, true)
		if checkErr != nil {
			return checkErr
		}

		fmt.Printf("Checked item %s\n", hash)

	case "uncheck":
		if len(args) < 3 {
			return fmt.Errorf("usage: threads todo <id> uncheck <hash>")
		}
		hash := args[2]

		// Check for ambiguous hash
		count := thread.CountMatchingItems(t.Content, "Todo", hash)
		if count == 0 {
			return fmt.Errorf("no checked item with hash '%s' found", hash)
		}
		if count > 1 {
			return fmt.Errorf("ambiguous hash '%s' matches %d items", hash, count)
		}

		var uncheckErr error
		t.Content, uncheckErr = thread.SetTodoChecked(t.Content, hash, false)
		if uncheckErr != nil {
			return uncheckErr
		}

		fmt.Printf("Unchecked item %s\n", hash)

	case "remove":
		if len(args) < 3 {
			return fmt.Errorf("usage: threads todo <id> remove <hash>")
		}
		hash := args[2]

		// Check for ambiguous hash
		count := thread.CountMatchingItems(t.Content, "Todo", hash)
		if count == 0 {
			return fmt.Errorf("no item with hash '%s' found", hash)
		}
		if count > 1 {
			return fmt.Errorf("ambiguous hash '%s' matches %d items", hash, count)
		}

		var removeErr error
		t.Content, removeErr = thread.RemoveByHash(t.Content, "Todo", hash)
		if removeErr != nil {
			return removeErr
		}

		fmt.Printf("Removed item %s\n", hash)

	default:
		return fmt.Errorf("unknown action '%s'. Use: add, check, uncheck, remove", action)
	}

	if err := t.Write(); err != nil {
		return err
	}

	if todoCommit {
		msg := todoMsg
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
