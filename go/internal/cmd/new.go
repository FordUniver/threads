package cmd

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/spf13/cobra"

	"git.zib.de/cspiegel/threads/internal/git"
	"git.zib.de/cspiegel/threads/internal/thread"
	"git.zib.de/cspiegel/threads/internal/workspace"
)

var (
	newStatus string
	newDesc   string
	newBody   string
	newCommit bool
	newMsg    string
)

var newCmd = &cobra.Command{
	Use:   "new [path] <title>",
	Short: "Create a new thread",
	Long: `Create a new thread at the specified level.

If path is omitted, the level is inferred from the current directory.
Use "." for workspace level.`,
	Args: cobra.RangeArgs(1, 2),
	RunE: runNew,
}

func init() {
	newCmd.Flags().StringVar(&newStatus, "status", "idea", "Initial status")
	newCmd.Flags().StringVar(&newDesc, "desc", "", "One-line description")
	newCmd.Flags().StringVar(&newBody, "body", "", "Initial body content")
	newCmd.Flags().BoolVar(&newCommit, "commit", false, "Commit after creating")
	newCmd.Flags().StringVarP(&newMsg, "m", "m", "", "Commit message")
}

func runNew(cmd *cobra.Command, args []string) error {
	ws := getWorkspace()

	var path, title string
	if len(args) == 2 {
		path = args[0]
		title = args[1]
	} else {
		title = args[0]
		// Infer path from cwd
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}
		path = cwd
	}

	if title == "" {
		return fmt.Errorf("title is required")
	}

	if !thread.IsValidStatus(newStatus) {
		return fmt.Errorf("invalid status '%s'. Must be one of: idea, planning, active, blocked, paused, resolved, superseded, deferred", newStatus)
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

	// Determine scope
	scope, err := workspace.InferScope(ws, path)
	if err != nil {
		return err
	}

	// Generate ID
	id, err := workspace.GenerateID(ws)
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

	relPath, _ := filepath.Rel(ws, threadPath)
	fmt.Printf("Created %s: %s\n", scope.LevelDesc, id)
	fmt.Printf("  → %s\n", relPath)

	if newBody == "" {
		fmt.Fprintln(os.Stderr, "Hint: Add body with: echo \"content\" | threads body", id, "--set")
	}

	// Commit if requested
	if newCommit {
		msg := newMsg
		if msg == "" {
			msg = git.GenerateCommitMessage(ws, []string{threadPath})
		}
		if err := git.AutoCommit(ws, threadPath, msg); err != nil {
			return err
		}
	} else {
		fmt.Printf("Note: Thread %s has uncommitted changes. Use 'threads commit %s' when ready.\n", id, id)
	}

	return nil
}
