package cmd

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/output"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	newStatus string
	newDesc   string
	newBody   string
	newCommit bool
	newMsg    string
	newFormat string
	newJSON   bool
)

type newOutput struct {
	ID           string `json:"id" yaml:"id"`
	Path         string `json:"path" yaml:"path"`
	PathAbsolute string `json:"path_absolute" yaml:"path_absolute"`
}

var newCmd = &cobra.Command{
	Use:   "new [path] <title>",
	Short: "Create a new thread",
	Long: `Create a new thread at the specified level.

Path resolution:
  (none)  → PWD (current directory)
  .       → PWD (explicit)
  ./X/Y   → PWD-relative
  /X/Y    → Absolute
  X/Y     → Git-root-relative`,
	Args: cobra.RangeArgs(1, 2),
	RunE: runNew,
}

func init() {
	newCmd.Flags().StringVar(&newStatus, "status", "idea", "Initial status")
	newCmd.Flags().StringVar(&newDesc, "desc", "", "One-line description")
	newCmd.Flags().StringVar(&newBody, "body", "", "Initial body content")
	newCmd.Flags().BoolVar(&newCommit, "commit", false, "Commit after creating")
	newCmd.Flags().StringVarP(&newMsg, "m", "m", "", "Commit message")
	newCmd.Flags().StringVarP(&newFormat, "format", "f", "fancy", "Output format (fancy, plain, json, yaml)")
	newCmd.Flags().BoolVar(&newJSON, "json", false, "Output as JSON (shorthand for --format=json)")
}

func runNew(cmd *cobra.Command, args []string) error {
	// Determine output format
	var fmt_ output.Format
	if newJSON {
		fmt_ = output.JSON
	} else {
		fmt_ = output.ParseFormat(newFormat).Resolve()
	}

	gitRoot := getWorkspace()

	var pathArg, title string
	if len(args) == 2 {
		pathArg = args[0]
		title = args[1]
	} else {
		title = args[0]
		// No path argument, will use PWD
		pathArg = ""
	}

	if title == "" {
		return fmt.Errorf("title is required")
	}

	if !thread.IsValidStatus(newStatus) {
		return fmt.Errorf("invalid status '%s'. Must be one of: idea, planning, active, blocked, paused, resolved, superseded, deferred, rejected", newStatus)
	}

	// Warn if no description provided
	if newDesc == "" {
		fmt.Fprintln(os.Stderr, "Warning: No --desc provided. Add one with: threads update <id> --desc \"...\"")
	}

	// Slugify title
	slug := workspace.Slugify(title)
	if slug == "" {
		return fmt.Errorf("title produces empty slug")
	}

	// Read body from stdin if available and not provided via flag
	if newBody == "" {
		stat, _ := os.Stdin.Stat()
		if (stat.Mode() & os.ModeCharDevice) == 0 {
			data, err := io.ReadAll(os.Stdin)
			if err == nil {
				newBody = string(data)
			}
		}
	}

	// Determine scope using new path resolution
	scope, err := workspace.InferScope(gitRoot, pathArg)
	if err != nil {
		return err
	}

	// Generate ID
	id, err := workspace.GenerateID(gitRoot)
	if err != nil {
		return err
	}

	// Ensure threads directory exists
	if err := os.MkdirAll(scope.ThreadsDir, 0755); err != nil {
		return fmt.Errorf("creating threads directory: %w", err)
	}

	// Build file path
	filename := fmt.Sprintf("%s-%s.md", id, slug)
	threadPath := filepath.Join(scope.ThreadsDir, filename)

	// Check if file already exists
	if _, err := os.Stat(threadPath); err == nil {
		return fmt.Errorf("thread already exists: %s", threadPath)
	}

	// Generate content
	today := time.Now().Format("2006-01-02")
	timestamp := time.Now().Format("15:04")

	var sb strings.Builder
	sb.WriteString("---\n")
	sb.WriteString(fmt.Sprintf("id: %s\n", id))
	sb.WriteString(fmt.Sprintf("name: %s\n", title))
	sb.WriteString(fmt.Sprintf("desc: %s\n", newDesc))
	sb.WriteString(fmt.Sprintf("status: %s\n", newStatus))
	sb.WriteString("---\n\n")

	if newBody != "" {
		sb.WriteString(newBody)
		if !strings.HasSuffix(newBody, "\n") {
			sb.WriteString("\n")
		}
		sb.WriteString("\n")
	}

	sb.WriteString("## Todo\n\n")
	sb.WriteString("## Log\n\n")
	sb.WriteString(fmt.Sprintf("### %s\n\n", today))
	sb.WriteString(fmt.Sprintf("- **%s** Created thread.\n", timestamp))

	// Write file
	if err := os.WriteFile(threadPath, []byte(sb.String()), 0644); err != nil {
		return fmt.Errorf("writing thread file: %w", err)
	}

	// Display path relative to git root
	relPath := workspace.PathRelativeToGitRoot(gitRoot, threadPath)

	switch fmt_ {
	case output.Fancy, output.Plain:
		fmt.Printf("Created thread in %s: %s\n", scope.LevelDesc, id)
		fmt.Printf("  → %s\n", relPath)

		if newBody == "" {
			fmt.Fprintln(os.Stderr, "Hint: Add body with: echo \"content\" | threads body", id, "--set")
		}
	case output.JSON:
		out := newOutput{
			ID:           id,
			Path:         relPath,
			PathAbsolute: threadPath,
		}
		data, err := json.MarshalIndent(out, "", "  ")
		if err != nil {
			return fmt.Errorf("JSON serialization failed: %v", err)
		}
		fmt.Println(string(data))
	case output.YAML:
		out := newOutput{
			ID:           id,
			Path:         relPath,
			PathAbsolute: threadPath,
		}
		data, err := yaml.Marshal(out)
		if err != nil {
			return fmt.Errorf("YAML serialization failed: %v", err)
		}
		fmt.Print(string(data))
	}

	// Commit if requested
	if newCommit {
		msg := newMsg
		if msg == "" {
			msg = git.GenerateCommitMessage(gitRoot, []string{threadPath})
		}
		if err := git.AutoCommit(gitRoot, threadPath, msg); err != nil {
			return err
		}
	} else if fmt_ == output.Fancy || fmt_ == output.Plain {
		fmt.Printf("Note: Thread %s has uncommitted changes. Use 'threads commit %s' when ready.\n", id, id)
	}

	return nil
}
