package thread

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"gopkg.in/yaml.v3"
)

// Status constants
var (
	TerminalStatuses = []string{"resolved", "superseded", "deferred", "reject"}
	ActiveStatuses   = []string{"idea", "planning", "active", "blocked", "paused"}
	AllStatuses      = append(ActiveStatuses, TerminalStatuses...)
)

// Frontmatter represents the YAML frontmatter of a thread
type Frontmatter struct {
	ID     string `yaml:"id"`
	Name   string `yaml:"name"`
	Desc   string `yaml:"desc"`
	Status string `yaml:"status"`
}

// Thread represents a parsed thread file
type Thread struct {
	Path        string      // absolute file path
	Frontmatter Frontmatter // parsed YAML frontmatter
	Content     string      // full file content
	BodyStart   int         // byte offset where body starts (after frontmatter)
}

// Note represents an item in the Notes section
type Note struct {
	Hash string
	Text string
}

// TodoItem represents a checkbox item in the Todo section
type TodoItem struct {
	Hash    string
	Text    string
	Checked bool
}

// idPrefixRe matches ID-prefixed filenames like "abc123-slug-name.md"
var idPrefixRe = regexp.MustCompile(`^([0-9a-f]{6})-`)

// Parse reads and parses a thread file
func Parse(path string) (*Thread, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	t := &Thread{
		Path:    path,
		Content: string(content),
	}

	if err := t.parseFrontmatter(); err != nil {
		return nil, fmt.Errorf("parsing frontmatter: %w", err)
	}

	// Extract ID from filename if not in frontmatter
	if t.Frontmatter.ID == "" {
		t.Frontmatter.ID = ExtractIDFromPath(path)
	}

	return t, nil
}

// parseFrontmatter extracts and parses YAML frontmatter
func (t *Thread) parseFrontmatter() error {
	content := t.Content

	if !strings.HasPrefix(content, "---\n") {
		return fmt.Errorf("missing frontmatter delimiter")
	}

	// Find closing delimiter
	end := strings.Index(content[4:], "\n---")
	if end == -1 {
		return fmt.Errorf("unclosed frontmatter")
	}

	yamlContent := content[4 : 4+end]
	t.BodyStart = 4 + end + 4 // skip opening ---, yaml, closing ---, and newline

	if err := yaml.Unmarshal([]byte(yamlContent), &t.Frontmatter); err != nil {
		return err
	}

	return nil
}

// ExtractIDFromPath extracts the 6-char hex ID from a filename
func ExtractIDFromPath(path string) string {
	filename := filepath.Base(path)
	filename = strings.TrimSuffix(filename, ".md")

	if m := idPrefixRe.FindStringSubmatch(filename); len(m) > 1 {
		return m[1]
	}
	return ""
}

// ExtractNameFromPath extracts the human-readable name from a filename
func ExtractNameFromPath(path string) string {
	filename := filepath.Base(path)
	filename = strings.TrimSuffix(filename, ".md")

	if m := idPrefixRe.FindStringSubmatch(filename); len(m) > 1 {
		return filename[7:] // skip "abc123-"
	}
	return filename
}

// ID returns the thread ID
func (t *Thread) ID() string {
	return t.Frontmatter.ID
}

// Name returns the thread name/title
func (t *Thread) Name() string {
	return t.Frontmatter.Name
}

// Status returns the thread status
func (t *Thread) Status() string {
	return t.Frontmatter.Status
}

// BaseStatus returns status without reason suffix (e.g., "blocked (waiting)" -> "blocked")
func (t *Thread) BaseStatus() string {
	return BaseStatus(t.Frontmatter.Status)
}

// BaseStatus strips reason suffix from status
func BaseStatus(status string) string {
	if idx := strings.Index(status, " ("); idx != -1 {
		return status[:idx]
	}
	return status
}

// IsTerminal returns true if the status is a terminal status
func IsTerminal(status string) bool {
	base := BaseStatus(status)
	for _, s := range TerminalStatuses {
		if s == base {
			return true
		}
	}
	return false
}

// IsValidStatus returns true if the status is valid
func IsValidStatus(status string) bool {
	base := BaseStatus(status)
	for _, s := range AllStatuses {
		if s == base {
			return true
		}
	}
	return false
}

// Body returns the content after frontmatter
func (t *Thread) Body() string {
	if t.BodyStart >= len(t.Content) {
		return ""
	}
	return t.Content[t.BodyStart:]
}

// SetFrontmatterField updates a frontmatter field and rewrites content
func (t *Thread) SetFrontmatterField(field, value string) error {
	switch field {
	case "id":
		t.Frontmatter.ID = value
	case "name":
		t.Frontmatter.Name = value
	case "desc":
		t.Frontmatter.Desc = value
	case "status":
		t.Frontmatter.Status = value
	default:
		return fmt.Errorf("unknown field: %s", field)
	}

	return t.rebuildContent()
}

// rebuildContent reconstructs file content from frontmatter and body
func (t *Thread) rebuildContent() error {
	var sb strings.Builder

	sb.WriteString("---\n")

	// Marshal frontmatter
	fm, err := yaml.Marshal(&t.Frontmatter)
	if err != nil {
		return err
	}
	sb.Write(fm)
	sb.WriteString("---\n")

	// Preserve body (BodyStart points to content after closing ---\n)
	if t.BodyStart < len(t.Content) {
		sb.WriteString(t.Content[t.BodyStart:])
	}

	t.Content = sb.String()
	return nil
}

// Write saves the thread to disk
func (t *Thread) Write() error {
	return os.WriteFile(t.Path, []byte(t.Content), 0644)
}

// RelPath returns the path relative to workspace
func (t *Thread) RelPath(ws string) string {
	rel, err := filepath.Rel(ws, t.Path)
	if err != nil {
		return t.Path
	}
	return rel
}
