package cmd

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var validateCmd = &cobra.Command{
	Use:   "validate [path]",
	Short: "Validate thread files",
	Args:  cobra.MaximumNArgs(1),
	RunE:  runValidate,
}

func runValidate(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()
	var files []string

	if len(args) > 0 {
		target := args[0]
		// Check if it's a file path
		absPath := target
		if !filepath.IsAbs(target) {
			absPath = filepath.Join(ws, target)
		}
		files = []string{absPath}
	} else {
		var err error
		files, err = workspace.FindAllThreads(ws)
		if err != nil {
			return err
		}
	}

	errors := 0
	for _, file := range files {
		relPath, _ := filepath.Rel(ws, file)
		t, err := thread.Parse(file)

		var issues []string

		if err != nil {
			issues = append(issues, fmt.Sprintf("parse error: %v", err))
		} else {
			if t.Name() == "" {
				issues = append(issues, "missing name/title field")
			}
			if t.Status() == "" {
				issues = append(issues, "missing status field")
			} else if !thread.IsValidStatus(t.Status()) {
				issues = append(issues, fmt.Sprintf("invalid status '%s'", thread.BaseStatus(t.Status())))
			}
		}

		if len(issues) > 0 {
			fmt.Printf("WARN: %s: %s\n", relPath, strings.Join(issues, ", "))
			errors++
		} else {
			fmt.Printf("OK: %s\n", relPath)
		}
	}

	if errors > 0 {
		return fmt.Errorf("%d validation error(s)", errors)
	}
	return nil
}
