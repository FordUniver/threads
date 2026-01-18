package cmd

import (
	"encoding/json"
	"fmt"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"

	"git.zib.de/cspiegel/threads/internal/output"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	validateFormat string
	validateJSON   bool
)

var validateCmd = &cobra.Command{
	Use:   "validate [path]",
	Short: "Validate thread files",
	Args:  cobra.MaximumNArgs(1),
	RunE:  runValidate,
}

func init() {
	validateCmd.Flags().StringVarP(&validateFormat, "format", "f", "fancy", "Output format (fancy, plain, json, yaml)")
	validateCmd.Flags().BoolVar(&validateJSON, "json", false, "Output as JSON (shorthand for --format=json)")
}

type validationResult struct {
	Path   string   `json:"path" yaml:"path"`
	Valid  bool     `json:"valid" yaml:"valid"`
	Issues []string `json:"issues" yaml:"issues"`
}

func runValidate(cmd *cobra.Command, args []string) error {
	// Determine output format
	var fmt_ output.Format
	if validateJSON {
		fmt_ = output.FormatJSON
	} else {
		parsed, _ := output.ParseFormat(validateFormat)
		fmt_ = parsed.Resolve()
	}

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

	var results []validationResult
	errorCount := 0

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

		valid := len(issues) == 0
		if !valid {
			errorCount++
		}

		results = append(results, validationResult{
			Path:   relPath,
			Valid:  valid,
			Issues: issues,
		})
	}

	// Output based on format
	switch fmt_ {
	case output.FormatFancy, output.FormatPlain:
		for _, r := range results {
			if r.Valid {
				fmt.Printf("OK: %s\n", r.Path)
			} else {
				fmt.Printf("WARN: %s: %s\n", r.Path, strings.Join(r.Issues, ", "))
			}
		}
	case output.FormatJSON:
		data := map[string]interface{}{
			"total":   len(results),
			"errors":  errorCount,
			"results": results,
		}
		out, err := json.MarshalIndent(data, "", "  ")
		if err != nil {
			return fmt.Errorf("JSON serialization failed: %v", err)
		}
		fmt.Println(string(out))
	case output.FormatYAML:
		data := map[string]interface{}{
			"total":   len(results),
			"errors":  errorCount,
			"results": results,
		}
		out, err := yaml.Marshal(data)
		if err != nil {
			return fmt.Errorf("YAML serialization failed: %v", err)
		}
		fmt.Print(string(out))
	}

	if errorCount > 0 {
		return fmt.Errorf("%d validation error(s)", errorCount)
	}
	return nil
}
