package cmd

import (
	"fmt"
	"os"
	"sort"
	"strings"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	statsRecursive bool
)

var statsCmd = &cobra.Command{
	Use:   "stats [path]",
	Short: "Show thread count by status",
	Args:  cobra.MaximumNArgs(1),
	RunE:  runStats,
}

func init() {
	statsCmd.Flags().BoolVarP(&statsRecursive, "recursive", "r", false, "Include nested categories/projects")
}

func runStats(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()

	// Parse path filter
	var categoryFilter, projectFilter string
	if len(args) > 0 {
		pathFilter := args[0]
		info, err := os.Stat(fmt.Sprintf("%s/%s", ws, pathFilter))
		if err == nil && info.IsDir() {
			parts := strings.SplitN(pathFilter, "/", 2)
			categoryFilter = parts[0]
			if len(parts) > 1 {
				projectFilter = parts[1]
			}
		}
	}

	// Find all threads
	threads, err := workspace.FindAllThreads(ws)
	if err != nil {
		return err
	}

	counts := make(map[string]int)
	total := 0

	for _, path := range threads {
		category, project, _ := workspace.ParseThreadPath(ws, path)

		// Category filter
		if categoryFilter != "" && category != categoryFilter {
			continue
		}

		// Project filter
		if projectFilter != "" && project != projectFilter {
			continue
		}

		// Non-recursive: only threads at current hierarchy level
		if !statsRecursive {
			if projectFilter != "" {
				// At project level, count all
			} else if categoryFilter != "" {
				if project != "-" {
					continue
				}
			} else {
				if category != "-" {
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

	// Build scope description
	var levelDesc, pathSuffix string
	if projectFilter != "" && categoryFilter != "" {
		levelDesc = "project-level"
		pathSuffix = fmt.Sprintf(" (%s/%s)", categoryFilter, projectFilter)
	} else if categoryFilter != "" {
		levelDesc = "category-level"
		pathSuffix = fmt.Sprintf(" (%s)", categoryFilter)
	} else {
		levelDesc = "workspace-level"
	}

	recursiveSuffix := ""
	if statsRecursive {
		recursiveSuffix = " (including nested)"
	}

	fmt.Printf("Stats for %s threads%s%s\n\n", levelDesc, pathSuffix, recursiveSuffix)

	if total == 0 {
		fmt.Println("No threads found.")
		if !statsRecursive {
			fmt.Println("Hint: use -r to include nested categories/projects")
		}
		return nil
	}

	// Sort by count descending
	type statusCount struct {
		status string
		count  int
	}
	var sorted []statusCount
	for s, c := range counts {
		sorted = append(sorted, statusCount{s, c})
	}
	sort.Slice(sorted, func(i, j int) bool {
		return sorted[i].count > sorted[j].count
	})

	fmt.Println("| Status     | Count |")
	fmt.Println("|------------|-------|")
	for _, sc := range sorted {
		fmt.Printf("| %-10s | %5d |\n", sc.status, sc.count)
	}
	fmt.Println("|------------|-------|")
	fmt.Printf("| %-10s | %5d |\n", "Total", total)

	return nil
}
