package thread

import "testing"

func TestExtractIDFromPath(t *testing.T) {
	tests := []struct {
		path string
		want string
	}{
		{"abc123-my-thread.md", "abc123"},
		{"/path/to/abc123-my-thread.md", "abc123"},
		{"deadbe-another-one.md", "deadbe"},
		{"no-id-here.md", ""},
		{"ABC123-uppercase.md", ""}, // only lowercase hex
		{"ab123-too-short.md", ""},  // need 6 chars
		{"abc1234-too-long.md", ""},
	}

	for _, tt := range tests {
		got := ExtractIDFromPath(tt.path)
		if got != tt.want {
			t.Errorf("ExtractIDFromPath(%q) = %q, want %q", tt.path, got, tt.want)
		}
	}
}

func TestExtractNameFromPath(t *testing.T) {
	tests := []struct {
		path string
		want string
	}{
		{"abc123-my-thread.md", "my-thread"},
		{"/path/to/abc123-my-thread.md", "my-thread"},
		{"abc123-multi-word-name.md", "multi-word-name"},
		{"no-id-here.md", "no-id-here"},
	}

	for _, tt := range tests {
		got := ExtractNameFromPath(tt.path)
		if got != tt.want {
			t.Errorf("ExtractNameFromPath(%q) = %q, want %q", tt.path, got, tt.want)
		}
	}
}

func TestBaseStatus(t *testing.T) {
	tests := []struct {
		status string
		want   string
	}{
		{"active", "active"},
		{"blocked (waiting for review)", "blocked"},
		{"resolved (done)", "resolved"},
		{"paused (vacation)", "paused"},
		{"idea", "idea"},
	}

	for _, tt := range tests {
		got := BaseStatus(tt.status)
		if got != tt.want {
			t.Errorf("BaseStatus(%q) = %q, want %q", tt.status, got, tt.want)
		}
	}
}

func TestIsTerminal(t *testing.T) {
	tests := []struct {
		status string
		want   bool
	}{
		{"resolved", true},
		{"superseded", true},
		{"deferred", true},
		{"rejected", true},
		{"resolved (completed)", true},
		{"active", false},
		{"blocked", false},
		{"idea", false},
		{"planning", false},
		{"paused", false},
	}

	for _, tt := range tests {
		got := IsTerminal(tt.status)
		if got != tt.want {
			t.Errorf("IsTerminal(%q) = %v, want %v", tt.status, got, tt.want)
		}
	}
}

func TestIsValidStatus(t *testing.T) {
	tests := []struct {
		status string
		want   bool
	}{
		// Active statuses
		{"idea", true},
		{"planning", true},
		{"active", true},
		{"blocked", true},
		{"paused", true},
		// Terminal statuses
		{"resolved", true},
		{"superseded", true},
		{"deferred", true},
		{"rejected", true},
		// With reason suffix
		{"blocked (waiting)", true},
		{"resolved (done)", true},
		// Invalid
		{"invalid", false},
		{"done", false},
		{"", false},
	}

	for _, tt := range tests {
		got := IsValidStatus(tt.status)
		if got != tt.want {
			t.Errorf("IsValidStatus(%q) = %v, want %v", tt.status, got, tt.want)
		}
	}
}
