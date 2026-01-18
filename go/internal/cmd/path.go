package cmd

import (
	"fmt"
	"path/filepath"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/workspace"
)

var pathCmd = &cobra.Command{
	Use:               "path <id>",
	Short:             "Print thread file path",
	Args:              cobra.ExactArgs(1),
	ValidArgsFunction: completeThreadIDs,
	RunE:              runPath,
}

func runPath(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	ref := args[0]

	file, err := workspace.FindByRef(ws, ref)
	if err != nil {
		return err
	}

	absPath, err := filepath.Abs(file)
	if err != nil {
		// Fallback to the file path if Abs fails
		absPath = file
	}

	fmt.Println(absPath)
	return nil
}
