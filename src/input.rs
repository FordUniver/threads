//! Stdin input utilities.

use std::io::{self, IsTerminal, Read};

/// Check if stdin is connected to a terminal (TTY).
///
/// Returns true for interactive terminal input, false for piped input.
pub fn stdin_is_tty() -> bool {
    io::stdin().is_terminal()
}

/// Read from stdin if piped (not a terminal).
///
/// Returns the content read from stdin, or an empty string if stdin is a terminal.
/// When `trim` is true, leading and trailing whitespace is removed.
pub fn read_stdin(trim: bool) -> String {
    if !io::stdin().is_terminal() {
        let mut buffer = String::new();
        if io::stdin().read_to_string(&mut buffer).is_ok() {
            return if trim {
                buffer.trim().to_string()
            } else {
                buffer
            };
        }
    }
    String::new()
}
