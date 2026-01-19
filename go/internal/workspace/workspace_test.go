package workspace

import "testing"

func TestSlugify(t *testing.T) {
	tests := []struct {
		title string
		want  string
	}{
		{"Hello World", "hello-world"},
		{"My Feature Request", "my-feature-request"},
		{"Fix: bug in parser", "fix-bug-in-parser"},
		{"Remove   extra   spaces", "remove-extra-spaces"},
		{"Trailing hyphens---", "trailing-hyphens"},
		{"---Leading hyphens", "leading-hyphens"},
		{"Special!@#$%chars", "special-chars"},
		{"MixedCASE", "mixedcase"},
		{"already-kebab-case", "already-kebab-case"},
		{"123 numbers first", "123-numbers-first"},
	}

	for _, tt := range tests {
		got := Slugify(tt.title)
		if got != tt.want {
			t.Errorf("Slugify(%q) = %q, want %q", tt.title, got, tt.want)
		}
	}
}

func TestDeduplicate(t *testing.T) {
	// deduplicate expects sorted input (removes consecutive duplicates only)
	tests := []struct {
		input []string
		want  []string
	}{
		{[]string{"a", "b", "c"}, []string{"a", "b", "c"}},
		{[]string{"a", "a", "b"}, []string{"a", "b"}},
		{[]string{"a", "a", "b", "b", "c"}, []string{"a", "b", "c"}},
		{[]string{}, []string{}},
		{[]string{"only"}, []string{"only"}},
	}

	for _, tt := range tests {
		got := deduplicate(tt.input)
		if len(got) != len(tt.want) {
			t.Errorf("deduplicate(%v) = %v, want %v", tt.input, got, tt.want)
			continue
		}
		for i := range got {
			if got[i] != tt.want[i] {
				t.Errorf("deduplicate(%v) = %v, want %v", tt.input, got, tt.want)
				break
			}
		}
	}
}
