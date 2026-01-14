package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	listRecursive     bool
	listIncludeClosed bool
	listSearch        string
	listStatus        string
	listCategory      string
	listProject       string
	listJSON          bool
)

var listCmd = &cobra.Command{
	Use:   "list [path]",
	Short: "List threads",
	Long: `List threads at the specified level.

By default shows active threads at the current level only.
Use -r to include nested categories/projects.
Use --include-closed to include resolved/terminal threads.`,
	Args: cobra.MaximumNArgs(1),
	RunE: runList,
}

func init() {
	listCmd.Flags().BoolVarP(&listRecursive, "recursive", "r", false, "Include nested categories/projects")
	listCmd.Flags().BoolVar(&listIncludeClosed, "include-closed", false, "Include resolved/terminal threads")
	listCmd.Flags().StringVarP(&listSearch, "search", "s", "", "Search name/title/desc (substring)")
	listCmd.Flags().StringVar(&listStatus, "status", "", "Filter by status")
	listCmd.Flags().StringVarP(&listCategory, "category", "c", "", "Filter by category")
	listCmd.Flags().StringVarP(&listProject, "project", "p", "", "Filter by project")
	listCmd.Flags().BoolVar(&listJSON, "json", false, "Output as JSON")
}

type threadInfo struct {
	ID       string `json:"id"`
	Status   string `json:"status"`
	Category string `json:"category"`
	Project  string `json:"project"`
	Name     string `json:"name"`
	Title    string `json:"title"`
	Desc     string `json:"desc"`
}

func runList(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()

	// Parse path filter if provided
	pathFilter := ""
	if len(args) > 0 {
		pathFilter = args[0]
	}

	// If path filter provided, extract category/project from it
	if pathFilter != "" {
		info, err := os.Stat(fmt.Sprintf("%s/%s", ws, pathFilter))
		if err == nil && info.IsDir() {
			parts := strings.SplitN(pathFilter, "/", 2)
			listCategory = parts[0]
			if len(parts) > 1 {
				listProject = parts[1]
			}
		} else {
			// Treat as search filter
			listSearch = pathFilter
		}
	}

	// Find all threads
	threads, err := workspace.FindAllThreads(ws)
	if err != nil {
		return err
	}

	var results []threadInfo

	for _, path := range threads {
		t, err := thread.Parse(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "warning: failed to parse %s: %v\n", path, err)
			continue
		}

		category, project, name := workspace.ParseThreadPath(ws, path)
		status := t.Status()
		baseStatus := thread.BaseStatus(status)

		// Category filter
		if listCategory != "" && category != listCategory {
			continue
		}

		// Project filter
		if listProject != "" && project != listProject {
			continue
		}

		// Non-recursive: only threads at current hierarchy level
		if !listRecursive {
			if listProject != "" {
				// At project level, show all threads here
			} else if listCategory != "" {
				// At category level: only show category-level threads
				if project != "-" {
					continue
				}
			} else {
				// At workspace level: only show workspace-level threads
				if category != "-" {
					continue
				}
			}
		}

		// Status filter
		statusFlagSet := cmd.Flags().Changed("status")
		if statusFlagSet {
			// Status filter was explicitly provided
			if listStatus == "" {
				// Empty status value matches nothing
				continue
			}
			if !strings.Contains(","+listStatus+",", ","+baseStatus+",") {
				continue
			}
		} else {
			// No status filter: apply default terminal filtering
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

		results = append(results, threadInfo{
			ID:       t.ID(),
			Status:   baseStatus,
			Category: category,
			Project:  project,
			Name:     name,
			Title:    title,
			Desc:     t.Frontmatter.Desc,
		})
	}

	if listJSON {
		return outputJSON(results)
	}

	return outputTable(results, ws)
}

func outputJSON(results []threadInfo) error {
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	return enc.Encode(results)
}

func outputTable(results []threadInfo, ws string) error {
	// Build header description
	var levelDesc string
	var pathSuffix string

	if listProject != "" && listCategory != "" {
		levelDesc = "project-level"
		pathSuffix = fmt.Sprintf(" (%s/%s)", listCategory, listProject)
	} else if listCategory != "" {
		levelDesc = "category-level"
		pathSuffix = fmt.Sprintf(" (%s)", listCategory)
	} else {
		levelDesc = "workspace-level"
	}

	statusDesc := "active"
	if listStatus != "" {
		statusDesc = listStatus
	} else if listIncludeClosed {
		statusDesc = ""
	}

	recursiveSuffix := ""
	if listRecursive {
		recursiveSuffix = " (including nested)"
	}

	if statusDesc != "" {
		fmt.Printf("Showing %d %s %s threads%s%s\n", len(results), statusDesc, levelDesc, pathSuffix, recursiveSuffix)
	} else {
		fmt.Printf("Showing %d %s threads%s (all statuses)%s\n", len(results), levelDesc, pathSuffix, recursiveSuffix)
	}
	fmt.Println()

	if len(results) == 0 {
		if !listRecursive {
			fmt.Println("Hint: use -r to include nested categories/projects")
		}
		return nil
	}

	// Print table header
	fmt.Printf("%-6s %-10s %-18s %-22s %s\n", "ID", "STATUS", "CATEGORY", "PROJECT", "NAME")
	fmt.Printf("%-6s %-10s %-18s %-22s %s\n", "--", "------", "--------", "-------", "----")

	for _, t := range results {
		category := truncate(t.Category, 16)
		project := truncate(t.Project, 20)
		fmt.Printf("%-6s %-10s %-18s %-22s %s\n", t.ID, t.Status, category, project, t.Title)
	}

	return nil
}

func truncate(s string, max int) string {
	if len(s) <= max {
		return s
	}
	return s[:max-1] + "…"
}

// lsCmd is an alias for listCmd
var lsCmd = &cobra.Command{
	Use:   "ls [path]",
	Short: "List threads (alias for list)",
	Long:  listCmd.Long,
	Args:  cobra.MaximumNArgs(1),
	RunE:  runList,
}

func init() {
	// Share flags with listCmd
	lsCmd.Flags().BoolVarP(&listRecursive, "recursive", "r", false, "Include nested categories/projects")
	lsCmd.Flags().BoolVar(&listIncludeClosed, "include-closed", false, "Include resolved/terminal threads")
	lsCmd.Flags().StringVarP(&listSearch, "search", "s", "", "Search name/title/desc (substring)")
	lsCmd.Flags().StringVar(&listStatus, "status", "", "Filter by status (comma-separated)")
	lsCmd.Flags().BoolVar(&listJSON, "json", false, "Output as JSON")
}
