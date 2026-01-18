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
	statsRecursive bool
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
  X/Y     → Git-root-relative`,
	Args: cobra.MaximumNArgs(1),
	RunE: runStats,
}

func init() {
	statsCmd.Flags().BoolVarP(&statsRecursive, "recursive", "r", false, "Include nested directories")
	statsCmd.Flags().StringVarP(&statsFormat, "format", "f", "fancy", "Output format: fancy, plain, json, yaml")
	statsCmd.Flags().BoolVar(&statsJSON, "json", false, "Output as JSON (shorthand for --format=json)")
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

	// Find all threads
	threads, err := workspace.FindAllThreads(gitRoot)
	if err != nil {
		return err
	}

	counts := make(map[string]int)
	total := 0

	for _, path := range threads {
		relPath := workspace.ParseThreadPath(gitRoot, path)

		// Path filter: if not recursive, only show threads at the specified level
		if !statsRecursive {
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
		return statsOutputFancy(sorted, total, filterPath)
	case output.FormatPlain:
		return statsOutputPlain(sorted, total, gitRoot, filterPath)
	case output.FormatJSON:
		return statsOutputJSON(sorted, total, gitRoot, filterPath)
	case output.FormatYAML:
		return statsOutputYAML(sorted, total, gitRoot, filterPath)
	default:
		return statsOutputFancy(sorted, total, filterPath)
	}
}

func statsOutputFancy(sorted []sortedCount, total int, filterPath string) error {
	pathDesc := filterPath
	if filterPath == "." {
		pathDesc = "repo root"
	}

	recursiveSuffix := ""
	if statsRecursive {
		recursiveSuffix = " (recursive)"
	}

	fmt.Printf("Stats for threads in %s%s\n", pathDesc, recursiveSuffix)
	fmt.Println()

	if total == 0 {
		fmt.Println("No threads found.")
		if !statsRecursive {
			fmt.Println("Hint: use -r to include nested directories")
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

func statsOutputPlain(sorted []sortedCount, total int, gitRoot, filterPath string) error {
	pwd, _ := os.Getwd()
	fmt.Printf("PWD: %s\n", pwd)
	fmt.Printf("Git root: %s\n", gitRoot)
	fmt.Println()

	pathDesc := filterPath
	if filterPath == "." {
		pathDesc = "repo root"
	}

	recursiveSuffix := ""
	if statsRecursive {
		recursiveSuffix = " (recursive)"
	}

	fmt.Printf("Stats for threads in %s%s\n", pathDesc, recursiveSuffix)
	fmt.Println()

	if total == 0 {
		fmt.Println("No threads found.")
		if !statsRecursive {
			fmt.Println("Hint: use -r to include nested directories")
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
