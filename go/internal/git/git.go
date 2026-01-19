package git

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

// HasChanges checks if a file has uncommitted changes (staged, unstaged, or untracked)
func HasChanges(ws, relPath string) bool {
	// Check unstaged changes
	cmd := exec.Command("git", "-C", ws, "diff", "--quiet", "--", relPath)
	if err := cmd.Run(); err != nil {
		return true
	}

	// Check staged changes
	cmd = exec.Command("git", "-C", ws, "diff", "--cached", "--quiet", "--", relPath)
	if err := cmd.Run(); err != nil {
		return true
	}

	// Check if untracked
	if !IsTracked(ws, relPath) {
		return true
	}

	return false
}

// IsTracked checks if a file is tracked by git
func IsTracked(ws, relPath string) bool {
	cmd := exec.Command("git", "-C", ws, "ls-files", "--error-unmatch", relPath)
	return cmd.Run() == nil
}

// ExistsInHEAD checks if a file exists in HEAD
func ExistsInHEAD(ws, relPath string) bool {
	ref := "HEAD:" + relPath
	cmd := exec.Command("git", "-C", ws, "cat-file", "-e", ref)
	return cmd.Run() == nil
}

// Add stages files, skipping any that don't exist (assumed to be already-staged deletions)
func Add(ws string, files ...string) error {
	var existingFiles []string
	for _, f := range files {
		fullPath := f
		if !filepath.IsAbs(f) {
			fullPath = filepath.Join(ws, f)
		}
		if fileExists(fullPath) {
			existingFiles = append(existingFiles, f)
		}
		// Non-existent files are assumed to be deletions already staged
	}

	if len(existingFiles) == 0 {
		return nil
	}

	args := append([]string{"-C", ws, "add"}, existingFiles...)
	cmd := exec.Command("git", args...)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("git add failed: %s", string(output))
	}
	return nil
}

// Commit creates a commit with the given message
func Commit(ws string, files []string, message string) error {
	// Stage files
	if err := Add(ws, files...); err != nil {
		return err
	}

	// Commit
	args := []string{"-C", ws, "commit", "-m", message}
	args = append(args, files...)
	cmd := exec.Command("git", args...)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("git commit failed: %s", string(output))
	}
	return nil
}

// Push does git pull --rebase && git push
func Push(ws string) error {
	// Pull with rebase
	cmd := exec.Command("git", "-C", ws, "pull", "--rebase")
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("git pull --rebase failed: %s", string(output))
	}

	// Push
	cmd = exec.Command("git", "-C", ws, "push")
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("git push failed: %s", string(output))
	}

	return nil
}

// AutoCommit stages and commits a file locally (does not push)
func AutoCommit(ws, file, message string) error {
	relPath, err := filepath.Rel(ws, file)
	if err != nil {
		relPath = file
	}

	return Commit(ws, []string{relPath}, message)
}

// GenerateCommitMessage creates a conventional commit message for thread changes
func GenerateCommitMessage(ws string, files []string) string {
	var added, modified, deleted []string

	for _, file := range files {
		relPath, _ := filepath.Rel(ws, file)
		name := filepath.Base(file)
		name = strings.TrimSuffix(name, ".md")

		if ExistsInHEAD(ws, relPath) {
			// File exists in HEAD
			if fileExists(file) {
				modified = append(modified, name)
			} else {
				deleted = append(deleted, name)
			}
		} else {
			// File not in HEAD - it's new
			added = append(added, name)
		}
	}

	total := len(added) + len(modified) + len(deleted)

	if total == 1 {
		if len(added) == 1 {
			return "threads: add " + extractID(added[0])
		}
		if len(modified) == 1 {
			return "threads: update " + extractID(modified[0])
		}
		return "threads: remove " + extractID(deleted[0])
	}

	if total <= 3 {
		var ids []string
		for _, name := range append(append(added, modified...), deleted...) {
			ids = append(ids, extractID(name))
		}
		action := "update"
		if len(added) == total {
			action = "add"
		} else if len(deleted) == total {
			action = "remove"
		}
		return fmt.Sprintf("threads: %s %s", action, strings.Join(ids, " "))
	}

	action := "update"
	if len(added) == total {
		action = "add"
	} else if len(deleted) == total {
		action = "remove"
	}
	return fmt.Sprintf("threads: %s %d threads", action, total)
}

// extractID extracts the ID prefix from a filename like "abc123-slug-name"
func extractID(name string) string {
	if len(name) >= 6 && isHex(name[:6]) {
		return name[:6]
	}
	return name
}

func isHex(s string) bool {
	for _, c := range s {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
			return false
		}
	}
	return true
}

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// FindDeletedThreadFiles returns paths of deleted thread files that are staged or in working tree
// These are files matching .threads/*.md that show as deleted (D) in git status
func FindDeletedThreadFiles(ws string) ([]string, error) {
	cmd := exec.Command("git", "-C", ws, "status", "--porcelain")
	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("git status failed: %w", err)
	}

	var deleted []string
	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		if len(line) < 4 {
			continue
		}
		// Porcelain format: XY PATH
		// X = index status, Y = worktree status
		// D in either position means deleted
		indexStatus := line[0]
		worktreeStatus := line[1]
		path := line[3:]

		// Check if it's a deletion and matches thread file pattern
		if (indexStatus == 'D' || worktreeStatus == 'D') && isThreadPath(path) {
			deleted = append(deleted, filepath.Join(ws, path))
		}
	}

	return deleted, nil
}

// isThreadPath checks if a path looks like a thread file (.threads/*.md)
func isThreadPath(path string) bool {
	return strings.Contains(path, ".threads/") && strings.HasSuffix(path, ".md")
}
