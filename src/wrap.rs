//! Text wrapping utilities with ANSI escape code preservation.
//!
//! This module provides a stable interface for text wrapping that:
//! - Preserves ANSI color/style codes across line breaks
//! - Supports prefix-based indentation (bullets, timestamps, etc.)
//! - Can be swapped to a different implementation if needed
//!
//! The current implementation uses `wrap-ansi` under the hood.

use unicode_width::UnicodeWidthStr;

/// Wrap text to fit within a given width, preserving ANSI codes.
///
/// Returns a vector of lines, each fitting within `width` visible characters.
/// ANSI escape codes are preserved and correctly handled across line breaks.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let wrapped = wrap_ansi::wrap_ansi(text, width, None);
    wrapped.lines().map(|s| s.to_string()).collect()
}

/// Wrap text with a prefix on the first line and indentation on continuation lines.
///
/// # Arguments
/// * `prefix` - The prefix for the first line (e.g., "• ", "☐ ", "12m ")
/// * `content` - The text content to wrap
/// * `width` - Total available width including prefix
///
/// # Example
/// ```ignore
/// let lines = wrap_with_prefix("• ", "This is a long note", 20);
/// // Returns:
/// // ["• This is a long", "  note"]
/// ```
pub fn wrap_with_prefix(prefix: &str, content: &str, width: usize) -> Vec<String> {
    let prefix_width = visible_width(prefix);

    // Width available for content (subtract prefix/indent width)
    let content_width = width.saturating_sub(prefix_width);
    if content_width == 0 {
        return vec![format!("{}{}", prefix, content)];
    }

    // Wrap the content
    let wrapped_lines = wrap(content, content_width);

    // Build indent string (same visible width as prefix, but spaces)
    let indent: String = " ".repeat(prefix_width);

    // Assemble: prefix on first line, indent on rest
    wrapped_lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                format!("{}{}", prefix, line)
            } else {
                format!("{}{}", indent, line)
            }
        })
        .collect()
}

/// Calculate visible width of a string, ignoring ANSI escape codes.
///
/// ANSI escape sequences (e.g., `\x1b[32m` for green) have zero display width.
pub fn visible_width(s: &str) -> usize {
    let stripped = strip_ansi(s);
    stripped.width()
}

/// Strip ANSI escape codes from a string.
fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_simple() {
        let lines = wrap("hello world", 5);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "hello");
        assert_eq!(lines[1], "world");
    }

    #[test]
    fn test_wrap_with_prefix_basic() {
        let lines = wrap_with_prefix("• ", "one two three", 10);
        // "• " is 2 chars, leaving 8 for content
        // "one two three" should wrap
        assert!(lines[0].starts_with("• "));
        if lines.len() > 1 {
            assert!(lines[1].starts_with("  ")); // 2-space indent
        }
    }

    #[test]
    fn test_wrap_with_prefix_preserves_prefix_width() {
        let lines = wrap_with_prefix("12m ", "short", 20);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "12m short");
    }

    #[test]
    fn test_visible_width_ignores_ansi() {
        let plain = "hello";
        let colored = "\x1b[32mhello\x1b[0m"; // green "hello"

        assert_eq!(visible_width(plain), 5);
        assert_eq!(visible_width(colored), 5);
    }

    #[test]
    fn test_strip_ansi() {
        let colored = "\x1b[32mgreen\x1b[0m and \x1b[31mred\x1b[0m";
        assert_eq!(strip_ansi(colored), "green and red");
    }
}
