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
	listDown          *int
	listRecursive     bool
	listUp            *int
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
Use -d/--down to include subdirectories, -u/--up to include parent directories.
Use -r as an alias for --down (unlimited depth).
Use --include-closed to include resolved/terminal threads.`,
	Args: cobra.MaximumNArgs(1),
	RunE: runList,
}

var listDownVal int
var listUpVal int

func init() {
	listCmd.Flags().IntVarP(&listDownVal, "down", "d", -1, "Search subdirectories (N levels, 0=unlimited)")
	listCmd.Flags().BoolVarP(&listRecursive, "recursive", "r", false, "Alias for --down (unlimited depth)")
	listCmd.Flags().IntVarP(&listUpVal, "up", "u", -1, "Search parent directories (N levels, 0=to git root)")
	listCmd.Flags().BoolVar(&listIncludeClosed, "include-closed", false, "Include resolved/terminal threads")
	listCmd.Flags().StringVarP(&listSearch, "search", "s", "", "Search name/title/desc (substring)")
	listCmd.Flags().StringVar(&listStatus, "status", "", "Filter by status (comma-separated)")
	listCmd.Flags().StringVarP(&listFormat, "format", "f", "fancy", "Output format: fancy, plain, json, yaml")
	listCmd.Flags().BoolVar(&listJSON, "json", false, "Output as JSON (shorthand for --format=json)")
}

// searchDirection describes the search direction for output display.
type searchDirection struct {
	hasDown   bool
	downDepth int // -1 = unlimited, 0+ = specific depth
	hasUp     bool
	upDepth   int // -1 = unlimited (to git root), 0+ = specific depth
}

func (s *searchDirection) description() string {
	var parts []string

	if s.hasDown {
		if s.downDepth < 0 {
			parts = append(parts, "recursive")
		} else {
			parts = append(parts, fmt.Sprintf("down %d", s.downDepth))
		}
	}

	if s.hasUp {
		if s.upDepth < 0 {
			parts = append(parts, "up")
		} else {
			parts = append(parts, fmt.Sprintf("up %d", s.upDepth))
		}
	}

	if len(parts) == 0 {
		return ""
	}
	return fmt.Sprintf(" (%s)", strings.Join(parts, ", "))
}

func (s *searchDirection) isSearching() bool {
	return s.hasDown || s.hasUp
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
	startPath := filepath.Dir(scope.ThreadsDir)

	// Determine search direction: --down/-d takes priority, then -r as alias
	downSet := cmd.Flags().Changed("down")
	hasDown := downSet || listRecursive
	downDepth := -1 // unlimited by default
	if downSet && listDownVal > 0 {
		downDepth = listDownVal
	}

	upSet := cmd.Flags().Changed("up")
	hasUp := upSet
	upDepth := -1 // unlimited (to git root) by default
	if upSet && listUpVal > 0 {
		upDepth = listUpVal
	}

	// Build options
	options := workspace.NewFindOptions()

	if hasDown {
		depth := downDepth
		if depth < 0 {
			depth = 0 // 0 means unlimited in our convention
		}
		options = options.WithDown(&depth)
	}

	if hasUp {
		depth := upDepth
		if depth < 0 {
			depth = 0 // 0 means unlimited in our convention
		}
		options = options.WithUp(&depth)
	}

	// Track search direction for output
	searchDir := &searchDirection{
		hasDown:   hasDown,
		downDepth: downDepth,
		hasUp:     hasUp,
		upDepth:   upDepth,
	}

	// Find threads using options
	threads, err := workspace.FindThreadsWithOptions(startPath, gitRoot, options)
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

		// Path filter: if not searching, only show threads at the specified level
		if !searchDir.isSearching() {
			if relPath != filterPath {
				continue
			}
		}
		// Note: FindThreadsWithOptions already handles direction/depth filtering

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
		return outputFancy(results, gitRoot, filterPath, pwdRel, searchDir)
	case output.FormatPlain:
		return outputPlain(results, gitRoot, filterPath, pwdRel, searchDir)
	case output.FormatJSON:
		return outputJSON(results, gitRoot, pwdRel)
	case output.FormatYAML:
		return outputYAML(results, gitRoot, pwdRel)
	default:
		return outputFancy(results, gitRoot, filterPath, pwdRel, searchDir)
	}
}

func outputFancy(results []threadInfo, gitRoot, filterPath, pwdRel string, searchDir *searchDirection) error {
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

	searchSuffix := searchDir.description()

	fmt.Printf("Showing %d %sthreads%s\n", len(results), statusDesc, searchSuffix)
	fmt.Println()

	if len(results) == 0 {
		if !searchDir.isSearching() {
			fmt.Println("Hint: use -r to include nested directories, -u to search parents")
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

func outputPlain(results []threadInfo, gitRoot, filterPath, pwdRel string, searchDir *searchDirection) error {
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

	searchSuffix := searchDir.description()

	pwdSuffix := ""
	if filterPath == pwdRel {
		pwdSuffix = " ← PWD"
	}

	if statusDesc != "" {
		fmt.Printf("Showing %d %s threads in %s%s%s\n", len(results), statusDesc, pathDesc, searchSuffix, pwdSuffix)
	} else {
		fmt.Printf("Showing %d threads in %s (all statuses)%s%s\n", len(results), pathDesc, searchSuffix, pwdSuffix)
	}
	fmt.Println()

	if len(results) == 0 {
		if !searchDir.isSearching() {
			fmt.Println("Hint: use -r to include nested directories, -u to search parents")
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
