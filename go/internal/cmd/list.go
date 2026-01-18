package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"

	"git.zib.de/cspiegel/threads/internal/output"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	listRecursive     bool
	listIncludeClosed bool
	listSearch        string
	listStatus        string
	listFormat        string
	listJSON          bool
)

var listCmd = &cobra.Command{
	Use:     "list [path]",
	Aliases: []string{"ls"},
	Short:   "List threads",
	Long: `List threads at the specified level.

Path resolution:
  (none)  → PWD (current directory)
  .       → PWD (explicit)
  ./X/Y   → PWD-relative
  /X/Y    → Absolute
  X/Y     → Git-root-relative

By default shows active threads at the current level only.
Use -r to include nested directories.
Use --include-closed to include resolved/terminal threads.`,
	Args: cobra.MaximumNArgs(1),
	RunE: runList,
}

func init() {
	listCmd.Flags().BoolVarP(&listRecursive, "recursive", "r", false, "Include nested directories")
	listCmd.Flags().BoolVar(&listIncludeClosed, "include-closed", false, "Include resolved/terminal threads")
	listCmd.Flags().StringVarP(&listSearch, "search", "s", "", "Search name/title/desc (substring)")
	listCmd.Flags().StringVar(&listStatus, "status", "", "Filter by status (comma-separated)")
	listCmd.Flags().StringVarP(&listFormat, "format", "f", "fancy", "Output format: fancy, plain, json, yaml")
	listCmd.Flags().BoolVar(&listJSON, "json", false, "Output as JSON (shorthand for --format=json)")
}

type threadInfo struct {
	ID           string `json:"id" yaml:"id"`
	Status       string `json:"status" yaml:"status"`
	Path         string `json:"path" yaml:"path"`
	Name         string `json:"name" yaml:"name"`
	Title        string `json:"title" yaml:"title"`
	Desc         string `json:"desc" yaml:"desc"`
	PathAbsolute string `json:"path_absolute,omitempty" yaml:"path_absolute,omitempty"`
	IsPwd        bool   `json:"is_pwd,omitempty" yaml:"is_pwd,omitempty"`
}

func runList(cmd *cobra.Command, args []string) error {
	gitRoot := getWorkspace()

	// Determine output format (handle --json shorthand)
	var format output.Format
	if listJSON {
		format = output.FormatJSON
	} else {
		format, _ = output.ParseFormat(listFormat)
		format = format.Resolve()
	}

	// Parse path filter if provided
	pathArg := ""
	if len(args) > 0 {
		pathArg = args[0]
	}

	// Resolve the scope
	scope, err := workspace.InferScope(gitRoot, pathArg)
	if err != nil {
		return err
	}
	filterPath := scope.Path

	// Find all threads
	threads, err := workspace.FindAllThreads(gitRoot)
	if err != nil {
		return err
	}

	// Get PWD relative path for comparison
	pwdRel, _ := workspace.PWDRelativeToGitRoot(gitRoot)

	// Determine if we need absolute paths (for json/yaml)
	includeAbsolute := format == output.FormatJSON || format == output.FormatYAML

	var results []threadInfo

	for _, path := range threads {
		t, err := thread.Parse(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "warning: failed to parse %s: %v\n", path, err)
			continue
		}

		relPath := workspace.ParseThreadPath(gitRoot, path)
		status := t.Status()
		baseStatus := thread.BaseStatus(status)
		name := thread.ExtractNameFromPath(path)

		// Path filter: if not recursive, only show threads at the specified level
		if !listRecursive {
			if relPath != filterPath {
				continue
			}
		} else {
			// Recursive mode: show threads at or under the filter path
			if filterPath != "." {
				filterPrefix := filterPath
				if !strings.HasSuffix(filterPrefix, "/") {
					filterPrefix = filterPath + "/"
				}
				if relPath != filterPath && !strings.HasPrefix(relPath, filterPrefix) {
					continue
				}
			}
		}

		// Status filter
		statusFlagSet := cmd.Flags().Changed("status")
		if statusFlagSet {
			if listStatus == "" {
				continue
			}
			if !strings.Contains(","+listStatus+",", ","+baseStatus+",") {
				continue
			}
		} else {
			if !listIncludeClosed && thread.IsTerminal(status) {
				continue
			}
		}

		// Search filter
		if listSearch != "" {
			searchLower := strings.ToLower(listSearch)
			nameLower := strings.ToLower(name)
			titleLower := strings.ToLower(t.Name())
			descLower := strings.ToLower(t.Frontmatter.Desc)

			if !strings.Contains(nameLower, searchLower) &&
				!strings.Contains(titleLower, searchLower) &&
				!strings.Contains(descLower, searchLower) {
				continue
			}
		}

		// Use title if available, else humanize name
		title := t.Name()
		if title == "" {
			title = strings.ReplaceAll(name, "-", " ")
		}

		isPwd := relPath == pwdRel

		info := threadInfo{
			ID:     t.ID(),
			Status: baseStatus,
			Path:   relPath,
			Name:   name,
			Title:  title,
			Desc:   t.Frontmatter.Desc,
			IsPwd:  isPwd,
		}

		if includeAbsolute {
			info.PathAbsolute = path
		}

		results = append(results, info)
	}

	switch format {
	case output.FormatFancy:
		return outputFancy(results, gitRoot, filterPath, pwdRel)
	case output.FormatPlain:
		return outputPlain(results, gitRoot, filterPath, pwdRel)
	case output.FormatJSON:
		return outputJSON(results, gitRoot, pwdRel)
	case output.FormatYAML:
		return outputYAML(results, gitRoot, pwdRel)
	default:
		return outputFancy(results, gitRoot, filterPath, pwdRel)
	}
}

func outputFancy(results []threadInfo, gitRoot, filterPath, pwdRel string) error {
	// Fancy header: repo-name (rel/path/to/pwd)
	repoName := filepath.Base(gitRoot)

	pathDesc := ""
	if filterPath != "." {
		pathDesc = fmt.Sprintf(" (%s)", filterPath)
	}

	pwdMarker := ""
	if filterPath == pwdRel {
		pwdMarker = " ← PWD"
	}

	fmt.Printf("%s%s%s\n", repoName, pathDesc, pwdMarker)
	fmt.Println()

	statusDesc := "active "
	if listStatus != "" {
		statusDesc = listStatus + " "
	} else if listIncludeClosed {
		statusDesc = ""
	}

	recursiveSuffix := ""
	if listRecursive {
		recursiveSuffix = " (recursive)"
	}

	fmt.Printf("Showing %d %sthreads%s\n", len(results), statusDesc, recursiveSuffix)
	fmt.Println()

	if len(results) == 0 {
		if !listRecursive {
			fmt.Println("Hint: use -r to include nested directories")
		}
		return nil
	}

	// Print table header
	fmt.Printf("%-6s %-10s %-24s %s\n", "ID", "STATUS", "PATH", "NAME")
	fmt.Printf("%-6s %-10s %-24s %s\n", "--", "------", "----", "----")

	for _, t := range results {
		pathDisplay := truncate(t.Path, 22)
		marker := ""
		if t.IsPwd {
			marker = " ←"
		}
		fmt.Printf("%-6s %-10s %-24s %s%s\n", t.ID, t.Status, pathDisplay, t.Title, marker)
	}

	return nil
}

func outputPlain(results []threadInfo, gitRoot, filterPath, pwdRel string) error {
	// Plain header: explicit context
	pwd, _ := os.Getwd()
	fmt.Printf("PWD: %s\n", pwd)
	fmt.Printf("Git root: %s\n", gitRoot)
	fmt.Printf("PWD (git-relative): %s\n", pwdRel)
	fmt.Println()

	pathDesc := filterPath
	if filterPath == "." {
		pathDesc = "repo root"
	}

	statusDesc := "active"
	if listStatus != "" {
		statusDesc = listStatus
	} else if listIncludeClosed {
		statusDesc = ""
	}

	recursiveSuffix := ""
	if listRecursive {
		recursiveSuffix = " (recursive)"
	}

	pwdSuffix := ""
	if filterPath == pwdRel {
		pwdSuffix = " ← PWD"
	}

	if statusDesc != "" {
		fmt.Printf("Showing %d %s threads in %s%s%s\n", len(results), statusDesc, pathDesc, recursiveSuffix, pwdSuffix)
	} else {
		fmt.Printf("Showing %d threads in %s (all statuses)%s%s\n", len(results), pathDesc, recursiveSuffix, pwdSuffix)
	}
	fmt.Println()

	if len(results) == 0 {
		if !listRecursive {
			fmt.Println("Hint: use -r to include nested directories")
		}
		return nil
	}

	// Print table header
	fmt.Printf("%-6s %-10s %-24s %s\n", "ID", "STATUS", "PATH", "NAME")
	fmt.Printf("%-6s %-10s %-24s %s\n", "--", "------", "----", "----")

	for _, t := range results {
		pathDisplay := truncate(t.Path, 22)
		pwdMarker := ""
		if t.IsPwd {
			pwdMarker = " ← PWD"
		}
		fmt.Printf("%-6s %-10s %-24s %s%s\n", t.ID, t.Status, pathDisplay, t.Title, pwdMarker)
	}

	return nil
}

func outputJSON(results []threadInfo, gitRoot, pwdRel string) error {
	pwd, _ := os.Getwd()

	type jsonOutput struct {
		PWD         string       `json:"pwd"`
		GitRoot     string       `json:"git_root"`
		PwdRelative string       `json:"pwd_relative"`
		Threads     []threadInfo `json:"threads"`
	}

	output := jsonOutput{
		PWD:         pwd,
		GitRoot:     gitRoot,
		PwdRelative: pwdRel,
		Threads:     results,
	}

	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	return enc.Encode(output)
}

func outputYAML(results []threadInfo, gitRoot, pwdRel string) error {
	pwd, _ := os.Getwd()

	type yamlOutput struct {
		PWD         string       `yaml:"pwd"`
		GitRoot     string       `yaml:"git_root"`
		PwdRelative string       `yaml:"pwd_relative"`
		Threads     []threadInfo `yaml:"threads"`
	}

	output := yamlOutput{
		PWD:         pwd,
		GitRoot:     gitRoot,
		PwdRelative: pwdRel,
		Threads:     results,
	}

	enc := yaml.NewEncoder(os.Stdout)
	enc.SetIndent(2)
	return enc.Encode(output)
}

func truncate(s string, max int) string {
	if len(s) <= max {
		return s
	}
	return s[:max-1] + "…"
}
