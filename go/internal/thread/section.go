package thread

import (
	"crypto/md5"
	"encoding/hex"
	"fmt"
	"regexp"
	"strings"
	"time"
)

// sectionRe matches section headers like "## Body", "## Notes", etc.
var sectionRe = regexp.MustCompile(`(?m)^## (\w+)`)

// hashCommentRe matches hash comments like "<!-- abc1 -->"
var hashCommentRe = regexp.MustCompile(`<!--\s*([a-f0-9]{4})\s*-->`)

// todoItemRe matches todo items like "- [ ] item" or "- [x] item"
var todoItemRe = regexp.MustCompile(`^- \[([ x])\] (.+?)\s*(<!--\s*[a-f0-9]{4}\s*-->)?$`)

// ExtractSection returns the content of a section (between ## Name and next ## or EOF)
func ExtractSection(content, name string) string {
	pattern := fmt.Sprintf(`(?ms)^## %s\n(.+?)(?:^## |\z)`, regexp.QuoteMeta(name))
	re := regexp.MustCompile(pattern)
	match := re.FindStringSubmatch(content)
	if len(match) < 2 {
		return ""
	}
	return strings.TrimSpace(match[1])
}

// ReplaceSection replaces the content of a section
func ReplaceSection(content, name, newContent string) string {
	pattern := fmt.Sprintf(`(?ms)(^## %s\n)(.+?)(^## |\z)`, regexp.QuoteMeta(name))
	re := regexp.MustCompile(pattern)

	if !re.MatchString(content) {
		// Section doesn't exist - handled by caller
		return content
	}

	return re.ReplaceAllString(content, fmt.Sprintf("${1}\n%s\n\n${3}", newContent))
}

// AppendToSection appends content to a section
func AppendToSection(content, name, addition string) string {
	sectionContent := ExtractSection(content, name)
	newContent := strings.TrimSpace(sectionContent)
	if newContent != "" {
		newContent += "\n"
	}
	newContent += addition
	return ReplaceSection(content, name, newContent)
}

// EnsureSection creates a section if it doesn't exist, placing it before another section
func EnsureSection(content, name, before string) string {
	pattern := fmt.Sprintf(`(?m)^## %s`, regexp.QuoteMeta(name))
	if regexp.MustCompile(pattern).MatchString(content) {
		return content
	}

	beforePattern := fmt.Sprintf(`(?m)(^## %s)`, regexp.QuoteMeta(before))
	beforeRe := regexp.MustCompile(beforePattern)

	if beforeRe.MatchString(content) {
		return beforeRe.ReplaceAllString(content, fmt.Sprintf("## %s\n\n$1", name))
	}

	// If before section doesn't exist, append at end
	return content + fmt.Sprintf("\n## %s\n\n", name)
}

// GenerateHash creates a 4-character hash for an item
func GenerateHash(text string) string {
	data := fmt.Sprintf("%s%d", text, time.Now().UnixNano())
	hash := md5.Sum([]byte(data))
	return hex.EncodeToString(hash[:])[:4]
}

// InsertLogEntry adds a timestamped entry to the Log section
func InsertLogEntry(content, entry string) string {
	today := time.Now().Format("2006-01-02")
	timestamp := time.Now().Format("15:04")
	bulletEntry := fmt.Sprintf("- **%s** %s", timestamp, entry)
	heading := fmt.Sprintf("### %s", today)

	// Check if today's heading exists
	todayPattern := regexp.MustCompile(fmt.Sprintf(`(?m)^### %s`, regexp.QuoteMeta(today)))
	if todayPattern.MatchString(content) {
		// Insert after today's heading
		pattern := fmt.Sprintf(`(?m)(^### %s\n)`, regexp.QuoteMeta(today))
		re := regexp.MustCompile(pattern)
		return re.ReplaceAllString(content, fmt.Sprintf("${1}\n%s\n", bulletEntry))
	}

	// Check if Log section exists
	logPattern := regexp.MustCompile(`(?m)^## Log`)
	if logPattern.MatchString(content) {
		// Insert new heading after ## Log
		return logPattern.ReplaceAllString(content, fmt.Sprintf("## Log\n\n%s\n\n%s", heading, bulletEntry))
	}

	// No Log section - append one
	return content + fmt.Sprintf("\n## Log\n\n%s\n\n%s\n", heading, bulletEntry)
}

// AddNote adds a note to the Notes section with a hash comment
func AddNote(content, text string) (string, string) {
	// Ensure Notes section exists
	content = EnsureSection(content, "Notes", "Todo")

	hash := GenerateHash(text)
	noteEntry := fmt.Sprintf("- %s  <!-- %s -->", text, hash)

	// Insert at top of Notes section
	pattern := regexp.MustCompile(`(?m)(^## Notes\n)`)
	newContent := pattern.ReplaceAllString(content, fmt.Sprintf("${1}\n%s\n", noteEntry))

	return newContent, hash
}

// RemoveByHash removes a line containing the specified hash comment from a section
func RemoveByHash(content, section, hash string) (string, error) {
	lines := strings.Split(content, "\n")
	inSection := false
	hashPattern := fmt.Sprintf("<!-- %s", hash)
	found := false

	var result []string
	for _, line := range lines {
		if strings.HasPrefix(line, "## "+section) {
			inSection = true
		} else if strings.HasPrefix(line, "## ") {
			inSection = false
		}

		if inSection && strings.Contains(line, hashPattern) && !found {
			found = true
			continue // skip this line
		}
		result = append(result, line)
	}

	if !found {
		return content, fmt.Errorf("no item with hash '%s' found", hash)
	}

	return strings.Join(result, "\n"), nil
}

// EditByHash replaces the text of an item by hash
func EditByHash(content, section, hash, newText string) (string, error) {
	lines := strings.Split(content, "\n")
	inSection := false
	hashPattern := fmt.Sprintf("<!-- %s", hash)
	found := false

	var result []string
	for _, line := range lines {
		if strings.HasPrefix(line, "## "+section) {
			inSection = true
		} else if strings.HasPrefix(line, "## ") {
			inSection = false
		}

		if inSection && strings.Contains(line, hashPattern) && !found {
			found = true
			// Extract hash from line and rebuild
			match := hashCommentRe.FindStringSubmatch(line)
			if len(match) > 1 {
				result = append(result, fmt.Sprintf("- %s  <!-- %s -->", newText, match[1]))
				continue
			}
		}
		result = append(result, line)
	}

	if !found {
		return content, fmt.Errorf("no item with hash '%s' found", hash)
	}

	return strings.Join(result, "\n"), nil
}

// AddTodoItem adds a checkbox item to the Todo section
func AddTodoItem(content, text string) (string, string) {
	hash := GenerateHash(text)
	todoEntry := fmt.Sprintf("- [ ] %s  <!-- %s -->", text, hash)

	// Insert at top of Todo section
	pattern := regexp.MustCompile(`(?m)(^## Todo\n)`)
	newContent := pattern.ReplaceAllString(content, fmt.Sprintf("${1}\n%s\n", todoEntry))

	return newContent, hash
}

// SetTodoChecked sets a todo item's checked state by hash
func SetTodoChecked(content, hash string, checked bool) (string, error) {
	lines := strings.Split(content, "\n")
	inTodo := false
	hashPattern := fmt.Sprintf("<!-- %s", hash)
	found := false

	var result []string
	for _, line := range lines {
		if strings.HasPrefix(line, "## Todo") {
			inTodo = true
		} else if strings.HasPrefix(line, "## ") {
			inTodo = false
		}

		if inTodo && strings.Contains(line, hashPattern) && !found {
			found = true
			if checked {
				line = strings.Replace(line, "- [ ]", "- [x]", 1)
			} else {
				line = strings.Replace(line, "- [x]", "- [ ]", 1)
			}
		}
		result = append(result, line)
	}

	if !found {
		return content, fmt.Errorf("no item with hash '%s' found", hash)
	}

	return strings.Join(result, "\n"), nil
}

// CountMatchingItems counts items matching a hash prefix in a section
func CountMatchingItems(content, section, hash string) int {
	lines := strings.Split(content, "\n")
	inSection := false
	hashPattern := fmt.Sprintf("<!-- %s", hash)
	count := 0

	for _, line := range lines {
		if strings.HasPrefix(line, "## "+section) {
			inSection = true
		} else if strings.HasPrefix(line, "## ") {
			inSection = false
		}

		if inSection && strings.Contains(line, hashPattern) {
			count++
		}
	}

	return count
}
