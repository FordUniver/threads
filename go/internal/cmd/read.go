package cmd

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/workspace"
)

var readCmd = &cobra.Command{
	Use:   "read <id>",
	Short: "Read thread content",
	Args:  cobra.ExactArgs(1),
	RunE:  runRead,
}

func runRead(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	ref := args[0]

	file, err := workspace.FindByRef(ws, ref)
	if err != nil {
		return err
	}

	content, err := os.ReadFile(file)
	if err != nil {
		return err
	}

	fmt.Print(string(content))
	return nil
}
