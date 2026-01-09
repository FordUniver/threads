package workspace

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"git.zib.de/cspiegel/threads/internal/thread"
)

// Find returns the workspace root from $WORKSPACE
func Find() (string, error) {
	ws := os.Getenv("WORKSPACE")
	if ws == "" {
		return "", fmt.Errorf("WORKSPACE environment variable not set")
	}
	ws = filepath.Clean(ws)
	if _, err := os.Stat(ws); os.IsNotExist(err) {
		return "", fmt.Errorf("WORKSPACE directory does not exist: %s", ws)
	}
	return ws, nil
}

// FindAllThreads returns all thread file paths in the workspace
func FindAllThreads(ws string) ([]string, error) {
	var threads []string

	patterns := []string{
		filepath.Join(ws, ".threads", "*.md"),
		filepath.Join(ws, "*", ".threads", "*.md"),
		filepath.Join(ws, "*", "*", ".threads", "*.md"),
	}

	for _, pattern := range patterns {
		matches, err := filepath.Glob(pattern)
		if err != nil {
			return nil, err
		}
		for _, m := range matches {
			// Skip archive directories
			if strings.Contains(m, "/archive/") {
				continue
			}
			threads = append(threads, m)
		}
	}

	sort.Strings(threads)
	return threads, nil
}

// Scope represents thread placement information
type Scope struct {
	ThreadsDir string // path to .threads directory
	Category   string // category name or "-" for workspace level
	Project    string // project name or "-" for category/workspace level
	LevelDesc  string // human-readable description
}

// InferScope determines the threads directory and level from a path
func InferScope(ws, path string) (*Scope, error) {
	// Handle explicit "." for workspace level
	if path == "." {
		return &Scope{
			ThreadsDir: filepath.Join(ws, ".threads"),
			Category:   "-",
			Project:    "-",
			LevelDesc:  "workspace-level thread",
		}, nil
	}

	var absPath string

	// Resolve to absolute path
	if filepath.IsAbs(path) {
		absPath = path
	} else if info, err := os.Stat(filepath.Join(ws, path)); err == nil && info.IsDir() {
		absPath = filepath.Join(ws, path)
	} else if info, err := os.Stat(path); err == nil && info.IsDir() {
		absPath, _ = filepath.Abs(path)
	} else {
		return nil, fmt.Errorf("path not found: %s", path)
	}

	// Must be within workspace
	if !strings.HasPrefix(absPath, ws) {
		return &Scope{
			ThreadsDir: filepath.Join(ws, ".threads"),
			Category:   "-",
			Project:    "-",
			LevelDesc:  "workspace-level thread",
		}, nil
	}

	rel := strings.TrimPrefix(absPath, ws)
	rel = strings.TrimPrefix(rel, string(filepath.Separator))

	if rel == "" {
		return &Scope{
			ThreadsDir: filepath.Join(ws, ".threads"),
			Category:   "-",
			Project:    "-",
			LevelDesc:  "workspace-level thread",
		}, nil
	}

	parts := strings.SplitN(rel, string(filepath.Separator), 3)
	category := parts[0]
	project := "-"

	if len(parts) >= 2 && parts[1] != "" {
		project = parts[1]
	}

	if project == "-" {
		return &Scope{
			ThreadsDir: filepath.Join(ws, category, ".threads"),
			Category:   category,
			Project:    "-",
			LevelDesc:  fmt.Sprintf("category-level thread (%s)", category),
		}, nil
	}

	return &Scope{
		ThreadsDir: filepath.Join(ws, category, project, ".threads"),
		Category:   category,
		Project:    project,
		LevelDesc:  fmt.Sprintf("project-level thread (%s/%s)", category, project),
	}, nil
}

// ParseThreadPath extracts category, project, and name from a thread file path
func ParseThreadPath(ws, path string) (category, project, name string) {
	rel := strings.TrimPrefix(path, ws)
	rel = strings.TrimPrefix(rel, string(filepath.Separator))

	filename := filepath.Base(path)
	filename = strings.TrimSuffix(filename, ".md")

	// Extract name, stripping ID prefix if present
	name = thread.ExtractNameFromPath(path)
	if name == "" {
		name = filename
	}

	// Check if workspace-level
	if strings.HasPrefix(rel, ".threads/") {
		return "-", "-", name
	}

	// Extract category and project from path like: category/project/.threads/name.md
	parts := strings.Split(rel, string(filepath.Separator))
	if len(parts) >= 2 {
		category = parts[0]
		if parts[1] == ".threads" {
			project = "-"
		} else if len(parts) >= 3 {
			project = parts[1]
		}
	}

	return category, project, name
}

// GenerateID creates a unique 6-character hex ID
func GenerateID(ws string) (string, error) {
	existing := make(map[string]bool)

	threads, err := FindAllThreads(ws)
	if err != nil {
		return "", err
	}

	for _, t := range threads {
		if id := thread.ExtractIDFromPath(t); id != "" {
			existing[id] = true
		}
	}

	// Try to generate unique ID
	for i := 0; i < 10; i++ {
		bytes := make([]byte, 3)
		if _, err := rand.Read(bytes); err != nil {
			return "", err
		}
		id := hex.EncodeToString(bytes)
		if !existing[id] {
			return id, nil
		}
	}

	return "", fmt.Errorf("could not generate unique ID after 10 attempts")
}

// Slugify converts a title to kebab-case filename
func Slugify(title string) string {
	// Convert to lowercase
	s := strings.ToLower(title)

	// Replace non-alphanumeric with hyphens
	re := regexp.MustCompile(`[^a-z0-9]+`)
	s = re.ReplaceAllString(s, "-")

	// Clean up multiple hyphens
	re = regexp.MustCompile(`-+`)
	s = re.ReplaceAllString(s, "-")

	// Trim leading/trailing hyphens
	s = strings.Trim(s, "-")

	return s
}

// FindByRef locates a thread by ID or name (with fuzzy matching)
func FindByRef(ws, ref string) (string, error) {
	threads, err := FindAllThreads(ws)
	if err != nil {
		return "", err
	}

	// Fast path: exact ID match
	idRe := regexp.MustCompile(`^[0-9a-f]{6}$`)
	if idRe.MatchString(ref) {
		for _, t := range threads {
			if thread.ExtractIDFromPath(t) == ref {
				return t, nil
			}
		}
	}

	// Slow path: name matching
	var substringMatches []string
	refLower := strings.ToLower(ref)

	for _, t := range threads {
		name := thread.ExtractNameFromPath(t)

		// Exact name match
		if name == ref {
			return t, nil
		}

		// Substring match (case-insensitive)
		if strings.Contains(strings.ToLower(name), refLower) {
			substringMatches = append(substringMatches, t)
		}
	}

	if len(substringMatches) == 1 {
		return substringMatches[0], nil
	}

	if len(substringMatches) > 1 {
		var ids []string
		for _, m := range substringMatches {
			id := thread.ExtractIDFromPath(m)
			name := thread.ExtractNameFromPath(m)
			ids = append(ids, fmt.Sprintf("%s (%s)", id, name))
		}
		return "", fmt.Errorf("ambiguous reference '%s' matches %d threads: %s",
			ref, len(substringMatches), strings.Join(ids, ", "))
	}

	return "", fmt.Errorf("thread not found: %s", ref)
}
