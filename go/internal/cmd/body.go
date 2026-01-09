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
	bodySet    bool
	bodyAppend bool
	bodyCommit bool
	bodyMsg    string
)

var bodyCmd = &cobra.Command{
	Use:   "body <id>",
	Short: "Edit Body section (stdin for content)",
	Long: `Edit the Body section of a thread.

Content is read from stdin. Use --set to replace or --append to add.`,
	Args: cobra.ExactArgs(1),
	RunE: runBody,
}

func init() {
	bodyCmd.Flags().BoolVar(&bodySet, "set", false, "Replace body content")
	bodyCmd.Flags().BoolVar(&bodyAppend, "append", false, "Append to body content")
	bodyCmd.Flags().BoolVar(&bodyCommit, "commit", false, "Commit after editing")
	bodyCmd.Flags().StringVarP(&bodyMsg, "m", "m", "", "Commit message")
}

func runBody(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	ref := args[0]

	// Default to set mode
	if !bodySet && !bodyAppend {
		bodySet = true
	}

	// Read content from stdin
	var content string
	stat, _ := os.Stdin.Stat()
	if (stat.Mode() & os.ModeCharDevice) == 0 {
		data, err := io.ReadAll(os.Stdin)
		if err != nil {
			return err
		}
		content = string(data)
	}

	if content == "" {
		return fmt.Errorf("no content provided (use stdin)")
	}

	file, err := workspace.FindByRef(ws, ref)
	if err != nil {
		return err
	}

	t, err := thread.Parse(file)
	if err != nil {
		return err
	}

	if bodySet {
		t.Content = thread.ReplaceSection(t.Content, "Body", content)
	} else {
		t.Content = thread.AppendToSection(t.Content, "Body", content)
	}

	if err := t.Write(); err != nil {
		return err
	}

	mode := "set"
	if bodyAppend {
		mode = "append"
	}
	fmt.Printf("Body %s: %s\n", mode, file)

	if bodyCommit {
		msg := bodyMsg
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
