package workspace

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"git.zib.de/cspiegel/threads/internal/thread"
)

// Pre-compiled regexes for Slugify
var (
	nonAlphanumRe = regexp.MustCompile(`[^a-z0-9]+`)
	multiHyphenRe = regexp.MustCompile(`-+`)
	hexIDRe       = regexp.MustCompile(`^[0-9a-f]{6}$`)
)

// FindOptions contains options for finding threads with direction and boundary controls.
type FindOptions struct {
	// Down specifies subdirectory search. nil = no recursion, *int nil = unlimited, *int N = N levels
	Down *int
	// Up specifies parent directory search. nil = no up search, *int nil = to git root, *int N = N levels
	Up *int
	// NoGitBoundDown allows crossing git boundaries when searching down
	NoGitBoundDown bool
	// NoGitBoundUp allows crossing git boundaries when searching up
	NoGitBoundUp bool
}

// NewFindOptions creates FindOptions with default values.
func NewFindOptions() *FindOptions {
	return &FindOptions{}
}

// WithDown enables searching subdirectories with optional depth limit.
// Pass nil for unlimited depth, or a pointer to a specific depth.
func (o *FindOptions) WithDown(depth *int) *FindOptions {
	o.Down = depth
	return o
}

// WithUp enables searching parent directories with optional depth limit.
// Pass nil for unlimited (to git root), or a pointer to a specific depth.
func (o *FindOptions) WithUp(depth *int) *FindOptions {
	o.Up = depth
	return o
}

// WithNoGitBoundDown allows crossing git boundaries when searching down.
func (o *FindOptions) WithNoGitBoundDown(value bool) *FindOptions {
	o.NoGitBoundDown = value
	return o
}

// WithNoGitBoundUp allows crossing git boundaries when searching up.
func (o *FindOptions) WithNoGitBoundUp(value bool) *FindOptions {
	o.NoGitBoundUp = value
	return o
}

// HasDown returns true if down searching is enabled.
func (o *FindOptions) HasDown() bool {
	return o.Down != nil
}

// HasUp returns true if up searching is enabled.
func (o *FindOptions) HasUp() bool {
	return o.Up != nil
}

// DownDepth returns the down depth limit, or -1 for unlimited.
func (o *FindOptions) DownDepth() int {
	if o.Down == nil {
		return 0
	}
	if *o.Down == 0 {
		return -1 // unlimited
	}
	return *o.Down
}

// UpDepth returns the up depth limit, or -1 for unlimited.
func (o *FindOptions) UpDepth() int {
	if o.Up == nil {
		return 0
	}
	if *o.Up == 0 {
		return -1 // unlimited
	}
	return *o.Up
}

// Find returns the git repository root from current directory.
// Returns an error if not in a git repository.
func Find() (string, error) {
	return FindGitRoot()
}

// FindGitRoot uses git rev-parse --show-toplevel to find the repository root.
func FindGitRoot() (string, error) {
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("not in a git repository. threads requires a git repo to define scope")
	}

	root := strings.TrimSpace(string(output))
	if root == "" {
		return "", fmt.Errorf("git root is empty")
	}

	return root, nil
}

// FindGitRootForPath finds the git root for a specific path.
func FindGitRootForPath(path string) (string, error) {
	cmd := exec.Command("git", "-C", path, "rev-parse", "--show-toplevel")
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("not in a git repository at: %s", path)
	}
	return strings.TrimSpace(string(output)), nil
}

// IsGitRoot checks if a directory contains a .git folder.
func IsGitRoot(path string) bool {
	info, err := os.Stat(filepath.Join(path, ".git"))
	return err == nil && info.IsDir()
}

// FindAllThreads returns all thread file paths within the git root.
// Scans recursively, respecting git boundaries (stops at nested git repos).
func FindAllThreads(gitRoot string) ([]string, error) {
	var threads []string
	if err := findThreadsRecursive(gitRoot, gitRoot, &threads); err != nil {
		return nil, err
	}
	sort.Strings(threads)
	return threads, nil
}

// findThreadsRecursive recursively finds .threads directories and collects thread files.
// Stops at nested git repositories (directories containing .git).
func findThreadsRecursive(dir, gitRoot string, threads *[]string) error {
	// Check for .threads directory here
	threadsDir := filepath.Join(dir, ".threads")
	if info, err := os.Stat(threadsDir); err == nil && info.IsDir() {
		entries, err := os.ReadDir(threadsDir)
		if err == nil {
			for _, entry := range entries {
				if entry.IsDir() {
					continue
				}
				if strings.HasSuffix(entry.Name(), ".md") {
					path := filepath.Join(threadsDir, entry.Name())
					// Skip archive subdirectory
					if !strings.Contains(path, "/archive/") {
						*threads = append(*threads, path)
					}
				}
			}
		}
	}

	// Recurse into subdirectories
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil // Silently skip unreadable directories
	}

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		name := entry.Name()

		// Skip hidden directories (except we already handled .threads)
		if strings.HasPrefix(name, ".") {
			continue
		}

		subdir := filepath.Join(dir, name)

		// Stop at nested git repos (unless it's the root itself)
		if subdir != gitRoot && IsGitRoot(subdir) {
			continue
		}

		findThreadsRecursive(subdir, gitRoot, threads)
	}

	return nil
}

// FindThreadsWithOptions finds threads with direction and boundary controls.
// This is the primary search function supporting --up, --down, and boundary flags.
func FindThreadsWithOptions(startPath, gitRoot string, options *FindOptions) ([]string, error) {
	var threads []string

	absStart, err := filepath.Abs(startPath)
	if err != nil {
		absStart = startPath
	}

	// Always collect threads at start_path
	collectThreadsAtPath(absStart, &threads)

	// Search down (subdirectories)
	if options.HasDown() {
		maxDepth := options.DownDepth()
		findThreadsDown(absStart, gitRoot, &threads, 0, maxDepth, options.NoGitBoundDown)
	}

	// Search up (parent directories)
	if options.HasUp() {
		maxDepth := options.UpDepth()
		findThreadsUp(absStart, gitRoot, &threads, 0, maxDepth, options.NoGitBoundUp)
	}

	// Sort and deduplicate
	sort.Strings(threads)
	threads = deduplicate(threads)

	return threads, nil
}

// collectThreadsAtPath collects threads from .threads directory at the given path.
func collectThreadsAtPath(dir string, threads *[]string) {
	threadsDir := filepath.Join(dir, ".threads")
	if info, err := os.Stat(threadsDir); err == nil && info.IsDir() {
		entries, err := os.ReadDir(threadsDir)
		if err == nil {
			for _, entry := range entries {
				if entry.IsDir() {
					continue
				}
				if strings.HasSuffix(entry.Name(), ".md") {
					path := filepath.Join(threadsDir, entry.Name())
					// Skip archive subdirectory
					if !strings.Contains(path, "/archive/") {
						*threads = append(*threads, path)
					}
				}
			}
		}
	}
}

// findThreadsDown recursively finds threads going down into subdirectories.
func findThreadsDown(dir, gitRoot string, threads *[]string, currentDepth, maxDepth int, crossGitBoundaries bool) {
	// Check depth limit (-1 means unlimited)
	if maxDepth >= 0 && currentDepth >= maxDepth {
		return
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		name := entry.Name()

		// Skip hidden directories
		if strings.HasPrefix(name, ".") {
			continue
		}

		subdir := filepath.Join(dir, name)

		// Check git boundary
		if !crossGitBoundaries && subdir != gitRoot && IsGitRoot(subdir) {
			continue
		}

		// Collect threads at this level
		collectThreadsAtPath(subdir, threads)

		// Continue recursing
		findThreadsDown(subdir, gitRoot, threads, currentDepth+1, maxDepth, crossGitBoundaries)
	}
}

// findThreadsUp finds threads going up into parent directories.
func findThreadsUp(dir, gitRoot string, threads *[]string, currentDepth, maxDepth int, crossGitBoundaries bool) {
	// Check depth limit (-1 means unlimited)
	if maxDepth >= 0 && currentDepth >= maxDepth {
		return
	}

	parent := filepath.Dir(dir)
	if parent == dir {
		return // reached root
	}

	absParent, _ := filepath.Abs(parent)
	absGitRoot, _ := filepath.Abs(gitRoot)

	// Check git boundary: stop at git root unless crossing is allowed
	if !crossGitBoundaries && !strings.HasPrefix(absParent, absGitRoot) {
		return
	}

	// Collect threads at parent
	collectThreadsAtPath(absParent, threads)

	// Continue up
	findThreadsUp(absParent, gitRoot, threads, currentDepth+1, maxDepth, crossGitBoundaries)
}

// deduplicate removes duplicate strings from a sorted slice.
func deduplicate(s []string) []string {
	if len(s) == 0 {
		return s
	}
	result := []string{s[0]}
	for i := 1; i < len(s); i++ {
		if s[i] != s[i-1] {
			result = append(result, s[i])
		}
	}
	return result
}

// Scope represents thread placement information.
// Path is relative to git root.
type Scope struct {
	ThreadsDir string // path to .threads directory (absolute)
	Path       string // path relative to git root (e.g., "src/models", "." for root)
	LevelDesc  string // human-readable description
}

// InferScope determines the threads directory and scope from a path specification.
//
// Path resolution rules:
// - "" or empty: PWD
// - ".": PWD (explicit)
// - "./X/Y": PWD-relative
// - "/X/Y": Absolute
// - "X/Y" (no leading ./ or /): Git-root-relative
func InferScope(gitRoot, pathArg string) (*Scope, error) {
	pwd, err := os.Getwd()
	if err != nil {
		return nil, fmt.Errorf("cannot get current directory: %w", err)
	}

	var targetPath string

	switch {
	case pathArg == "" || pathArg == ".":
		// No path argument or explicit ".": use PWD
		targetPath = pwd

	case strings.HasPrefix(pathArg, "./"):
		// PWD-relative path: ./X/Y
		rel := strings.TrimPrefix(pathArg, "./")
		targetPath = filepath.Join(pwd, rel)

	case strings.HasPrefix(pathArg, "/"):
		// Absolute path
		targetPath = pathArg

	default:
		// Git-root-relative path: X/Y
		targetPath = filepath.Join(gitRoot, pathArg)
	}

	// Clean and resolve path
	targetPath = filepath.Clean(targetPath)

	// Check if directory exists
	if info, err := os.Stat(targetPath); err != nil || !info.IsDir() {
		return nil, fmt.Errorf("path not found or not a directory: %s", targetPath)
	}

	// Get absolute paths for comparison
	absTarget, err := filepath.Abs(targetPath)
	if err != nil {
		absTarget = targetPath
	}
	absGitRoot, err := filepath.Abs(gitRoot)
	if err != nil {
		absGitRoot = gitRoot
	}

	// Verify target is within the git repo
	if !strings.HasPrefix(absTarget, absGitRoot) {
		return nil, fmt.Errorf("path must be within git repository: %s (git root: %s)",
			targetPath, gitRoot)
	}

	// Check if target is inside a nested git repo
	if absTarget != absGitRoot {
		checkPath := absTarget
		for checkPath != absGitRoot {
			if IsGitRoot(checkPath) {
				return nil, fmt.Errorf("path is inside a nested git repository at: %s. Use --no-git-bound to cross git boundaries",
					checkPath)
			}
			checkPath = filepath.Dir(checkPath)
		}
	}

	// Compute path relative to git root
	relPath, err := filepath.Rel(absGitRoot, absTarget)
	if err != nil || relPath == "" {
		relPath = "."
	}
	if relPath == "." {
		// Explicitly use "." for root
	}

	// Build description
	levelDesc := relPath
	if relPath == "." {
		levelDesc = "repo root"
	}

	// Build threads directory path
	threadsDir := filepath.Join(absTarget, ".threads")

	return &Scope{
		ThreadsDir: threadsDir,
		Path:       relPath,
		LevelDesc:  levelDesc,
	}, nil
}

// ParseThreadPath extracts the git-relative path component from a thread file path.
// Returns the path relative to git root (e.g., "src/models").
func ParseThreadPath(gitRoot, threadPath string) string {
	absGitRoot, _ := filepath.Abs(gitRoot)
	absPath, _ := filepath.Abs(threadPath)

	// Get path relative to git root
	rel, err := filepath.Rel(absGitRoot, absPath)
	if err != nil {
		return "."
	}

	// Extract the directory containing .threads
	// Pattern: <path>/.threads/file.md -> return <path>
	dir := filepath.Dir(rel)
	if strings.HasSuffix(dir, "/.threads") {
		parent := filepath.Dir(dir)
		if parent == "." || parent == "" {
			return "."
		}
		return parent
	}
	if dir == ".threads" {
		return "."
	}

	return "."
}

// PathRelativeToGitRoot returns the path relative to git root for display purposes.
func PathRelativeToGitRoot(gitRoot, path string) string {
	absGitRoot, _ := filepath.Abs(gitRoot)
	absPath, _ := filepath.Abs(path)

	rel, err := filepath.Rel(absGitRoot, absPath)
	if err != nil || rel == "" {
		return "."
	}
	return rel
}

// PWDRelativeToGitRoot returns the current working directory relative to git root.
func PWDRelativeToGitRoot(gitRoot string) (string, error) {
	pwd, err := os.Getwd()
	if err != nil {
		return "", err
	}
	return PathRelativeToGitRoot(gitRoot, pwd), nil
}

// GenerateID creates a unique 6-character hex ID.
func GenerateID(gitRoot string) (string, error) {
	existing := make(map[string]bool)

	threads, err := FindAllThreads(gitRoot)
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

// Slugify converts a title to kebab-case filename.
func Slugify(title string) string {
	// Convert to lowercase
	s := strings.ToLower(title)

	// Replace non-alphanumeric with hyphens
	s = nonAlphanumRe.ReplaceAllString(s, "-")

	// Clean up multiple hyphens
	s = multiHyphenRe.ReplaceAllString(s, "-")

	// Trim leading/trailing hyphens
	s = strings.Trim(s, "-")

	return s
}

// FindByRef locates a thread by ID or name (with fuzzy matching).
func FindByRef(gitRoot, ref string) (string, error) {
	threads, err := FindAllThreads(gitRoot)
	if err != nil {
		return "", err
	}

	// Fast path: exact ID match
	if hexIDRe.MatchString(ref) {
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
