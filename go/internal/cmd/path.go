package cmd

import (
	"encoding/json"
	"fmt"
	"path/filepath"

	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"

	"git.zib.de/cspiegel/threads/internal/output"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	pathFormat string
	pathJSON   bool
)

var pathCmd = &cobra.Command{
	Use:               "path <id>",
	Short:             "Print thread file path",
	Args:              cobra.ExactArgs(1),
	ValidArgsFunction: completeThreadIDs,
	RunE:              runPath,
}

func init() {
	pathCmd.Flags().StringVarP(&pathFormat, "format", "f", "fancy", "Output format (fancy, plain, json, yaml)")
	pathCmd.Flags().BoolVar(&pathJSON, "json", false, "Output as JSON (shorthand for --format=json)")
}

type pathOutput struct {
	Path         string `json:"path" yaml:"path"`
	PathAbsolute string `json:"path_absolute" yaml:"path_absolute"`
}

func runPath(cmd *cobra.Command, args []string) error {
	// Determine output format
	var fmt_ output.Format
	if pathJSON {
		fmt_ = output.JSON
	} else {
		fmt_ = output.ParseFormat(pathFormat).Resolve()
	}

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
	relPath := workspace.PathRelativeToGitRoot(ws, file)

	switch fmt_ {
	case output.Fancy, output.Plain:
		fmt.Println(absPath)
	case output.JSON:
		out := pathOutput{
			Path:         relPath,
			PathAbsolute: absPath,
		}
		data, err := json.MarshalIndent(out, "", "  ")
		if err != nil {
			return fmt.Errorf("JSON serialization failed: %v", err)
		}
		fmt.Println(string(data))
	case output.YAML:
		out := pathOutput{
			Path:         relPath,
			PathAbsolute: absPath,
		}
		data, err := yaml.Marshal(out)
		if err != nil {
			return fmt.Errorf("YAML serialization failed: %v", err)
		}
		fmt.Print(string(data))
	}

	return nil
}
