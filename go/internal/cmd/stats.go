package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"strings"

	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"

	"git.zib.de/cspiegel/threads/internal/output"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	statsDownVal   int
	statsRecursive bool
	statsUpVal     int
	statsFormat    string
	statsJSON      bool
)

var statsCmd = &cobra.Command{
	Use:   "stats [path]",
	Short: "Show thread count by status",
	Long: `Show thread count by status at the specified level.

Path resolution:
  (none)  → PWD (current directory)
  .       → PWD (explicit)
  ./X/Y   → PWD-relative
  /X/Y    → Absolute
  X/Y     → Git-root-relative

Use -d/--down to include subdirectories, -u/--up to include parent directories.
Use -r as an alias for --down (unlimited depth).`,
	Args: cobra.MaximumNArgs(1),
	RunE: runStats,
}

func init() {
	statsCmd.Flags().IntVarP(&statsDownVal, "down", "d", -1, "Search subdirectories (N levels, 0=unlimited)")
	statsCmd.Flags().BoolVarP(&statsRecursive, "recursive", "r", false, "Alias for --down (unlimited depth)")
	statsCmd.Flags().IntVarP(&statsUpVal, "up", "u", -1, "Search parent directories (N levels, 0=to git root)")
	statsCmd.Flags().StringVarP(&statsFormat, "format", "f", "fancy", "Output format: fancy, plain, json, yaml")
	statsCmd.Flags().BoolVar(&statsJSON, "json", false, "Output as JSON (shorthand for --format=json)")
}

// statsSearchDirection describes the search direction for stats output display.
type statsSearchDirection struct {
	hasDown   bool
	downDepth int
	hasUp     bool
	upDepth   int
}

func (s *statsSearchDirection) description() string {
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

func (s *statsSearchDirection) isSearching() bool {
	return s.hasDown || s.hasUp
}

type statusCount struct {
	Status string `json:"status" yaml:"status"`
	Count  int    `json:"count" yaml:"count"`
}

type sortedCount struct {
	Key   string
	Value int
}

func runStats(cmd *cobra.Command, args []string) error {
	gitRoot := getWorkspace()

	// Determine output format (handle --json shorthand)
	var format output.Format
	if statsJSON {
		format = output.FormatJSON
	} else {
		format, _ = output.ParseFormat(statsFormat)
		format = format.Resolve()
	}

	// Parse path filter
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
	startPath := scope.ThreadsDir[:len(scope.ThreadsDir)-len("/.threads")]

	// Determine search direction
	downSet := cmd.Flags().Changed("down")
	hasDown := downSet || statsRecursive
	downDepth := -1
	if downSet && statsDownVal > 0 {
		downDepth = statsDownVal
	}

	upSet := cmd.Flags().Changed("up")
	hasUp := upSet
	upDepth := -1
	if upSet && statsUpVal > 0 {
		upDepth = statsUpVal
	}

	// Build options
	options := workspace.NewFindOptions()

	if hasDown {
		depth := downDepth
		if depth < 0 {
			depth = 0
		}
		options = options.WithDown(&depth)
	}

	if hasUp {
		depth := upDepth
		if depth < 0 {
			depth = 0
		}
		options = options.WithUp(&depth)
	}

	// Track search direction for output
	searchDir := &statsSearchDirection{
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

	counts := make(map[string]int)
	total := 0

	for _, path := range threads {
		relPath := workspace.ParseThreadPath(gitRoot, path)

		// Path filter: if not searching, only count threads at the specified level
		if !searchDir.isSearching() {
			if relPath != filterPath {
				continue
			}
		}

		t, err := thread.Parse(path)
		if err != nil {
			continue
		}

		status := t.BaseStatus()
		if status == "" {
			status = "(none)"
		}

		counts[status]++
		total++
	}

	// Sort by count descending
	var sorted []sortedCount
	for k, v := range counts {
		sorted = append(sorted, sortedCount{k, v})
	}
	sort.Slice(sorted, func(i, j int) bool {
		return sorted[i].Value > sorted[j].Value
	})

	switch format {
	case output.FormatFancy:
		return statsOutputFancy(sorted, total, filterPath, searchDir)
	case output.FormatPlain:
		return statsOutputPlain(sorted, total, gitRoot, filterPath, searchDir)
	case output.FormatJSON:
		return statsOutputJSON(sorted, total, gitRoot, filterPath)
	case output.FormatYAML:
		return statsOutputYAML(sorted, total, gitRoot, filterPath)
	default:
		return statsOutputFancy(sorted, total, filterPath, searchDir)
	}
}

func statsOutputFancy(sorted []sortedCount, total int, filterPath string, searchDir *statsSearchDirection) error {
	pathDesc := filterPath
	if filterPath == "." {
		pathDesc = "repo root"
	}

	searchSuffix := searchDir.description()

	fmt.Printf("Stats for threads in %s%s\n", pathDesc, searchSuffix)
	fmt.Println()

	if total == 0 {
		fmt.Println("No threads found.")
		if !searchDir.isSearching() {
			fmt.Println("Hint: use -r to include nested directories, -u to search parents")
		}
		return nil
	}

	fmt.Println("| Status     | Count |")
	fmt.Println("|------------|-------|")
	for _, kv := range sorted {
		fmt.Printf("| %-10s | %5d |\n", kv.Key, kv.Value)
	}
	fmt.Println("|------------|-------|")
	fmt.Printf("| %-10s | %5d |\n", "Total", total)

	return nil
}

func statsOutputPlain(sorted []sortedCount, total int, gitRoot, filterPath string, searchDir *statsSearchDirection) error {
	pwd, _ := os.Getwd()
	fmt.Printf("PWD: %s\n", pwd)
	fmt.Printf("Git root: %s\n", gitRoot)
	fmt.Println()

	pathDesc := filterPath
	if filterPath == "." {
		pathDesc = "repo root"
	}

	searchSuffix := searchDir.description()

	fmt.Printf("Stats for threads in %s%s\n", pathDesc, searchSuffix)
	fmt.Println()

	if total == 0 {
		fmt.Println("No threads found.")
		if !searchDir.isSearching() {
			fmt.Println("Hint: use -r to include nested directories, -u to search parents")
		}
		return nil
	}

	fmt.Println("| Status     | Count |")
	fmt.Println("|------------|-------|")
	for _, kv := range sorted {
		fmt.Printf("| %-10s | %5d |\n", kv.Key, kv.Value)
	}
	fmt.Println("|------------|-------|")
	fmt.Printf("| %-10s | %5d |\n", "Total", total)

	return nil
}

func statsOutputJSON(sorted []sortedCount, total int, gitRoot, filterPath string) error {
	type jsonOutput struct {
		GitRoot string        `json:"git_root"`
		Path    string        `json:"path"`
		Counts  []statusCount `json:"counts"`
		Total   int           `json:"total"`
	}

	var counts []statusCount
	for _, kv := range sorted {
		counts = append(counts, statusCount{
			Status: kv.Key,
			Count:  kv.Value,
		})
	}

	output := jsonOutput{
		GitRoot: gitRoot,
		Path:    filterPath,
		Counts:  counts,
		Total:   total,
	}

	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	return enc.Encode(output)
}

func statsOutputYAML(sorted []sortedCount, total int, gitRoot, filterPath string) error {
	type yamlOutput struct {
		GitRoot string        `yaml:"git_root"`
		Path    string        `yaml:"path"`
		Counts  []statusCount `yaml:"counts"`
		Total   int           `yaml:"total"`
	}

	var counts []statusCount
	for _, kv := range sorted {
		counts = append(counts, statusCount{
			Status: kv.Key,
			Count:  kv.Value,
		})
	}

	output := yamlOutput{
		GitRoot: gitRoot,
		Path:    filterPath,
		Counts:  counts,
		Total:   total,
	}

	enc := yaml.NewEncoder(os.Stdout)
	enc.SetIndent(2)
	return enc.Encode(output)
}
