// Package output provides output formatting utilities with TTY auto-detection.
package output

import (
	"os"

	"golang.org/x/term"
)

// Format represents the output format.
type Format string

const (
	FormatFancy Format = "fancy"
	FormatPlain Format = "plain"
	FormatJSON  Format = "json"
	FormatYAML  Format = "yaml"
)

// ParseFormat parses a format string, returning an error for invalid values.
func ParseFormat(s string) (Format, error) {
	switch s {
	case "fancy", "":
		return FormatFancy, nil
	case "plain":
		return FormatPlain, nil
	case "json":
		return FormatJSON, nil
	case "yaml":
		return FormatYAML, nil
	default:
		return "", nil
	}
}

// Resolve applies TTY auto-detection: if format is Fancy but stdout is not a TTY, returns Plain.
func (f Format) Resolve() Format {
	if f == FormatFancy && !IsTTY() {
		return FormatPlain
	}
	return f
}

// IsTTY returns true if stdout is connected to a terminal.
func IsTTY() bool {
	return term.IsTerminal(int(os.Stdout.Fd()))
}
