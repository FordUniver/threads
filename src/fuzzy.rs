//! Lightweight fuzzy matching with scoring for ranking results.
//!
//! This module implements a simple subsequence matcher that assigns higher
//! scores to matches that are:
//! - earlier in the string (leading penalty)
//! - on word boundaries (boundary bonus)
//! - consecutive (consecutive bonus)
//!
//! The implementation is dependency-free to keep `threads` self-contained.

const NEG_INF: i64 = i64::MIN / 4;

// Scoring parameters (tuned for human-oriented ranking).
const SCORE_MATCH: i64 = 10;
const BONUS_START: i64 = 15;
const BONUS_BOUNDARY: i64 = 12;
const BONUS_CAMEL: i64 = 10;
const BONUS_CONSECUTIVE: i64 = 15;
const PENALTY_GAP: i64 = 1;
const PENALTY_LEADING: i64 = 1;

fn fold(c: char) -> char {
    c.to_ascii_lowercase()
}

fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '/' | '\\' | '-' | '_' | '.' | ':' | '@' | '#')
}

fn boundary_bonus(prev: Option<char>, current: char, idx: usize) -> i64 {
    if idx == 0 {
        return BONUS_START;
    }
    let Some(prev) = prev else {
        return 0;
    };
    if is_separator(prev) {
        return BONUS_BOUNDARY;
    }
    if prev.is_lowercase() && current.is_uppercase() {
        return BONUS_CAMEL;
    }
    0
}

fn compute_match_scores(haystack_chars: &[char]) -> Vec<i64> {
    let mut scores = Vec::with_capacity(haystack_chars.len());
    for (idx, &c) in haystack_chars.iter().enumerate() {
        let prev = if idx == 0 {
            None
        } else {
            Some(haystack_chars[idx - 1])
        };
        let b = boundary_bonus(prev, c, idx);
        scores.push(SCORE_MATCH + b);
    }
    scores
}

/// Fuzzy-match `needle` against `haystack` and return a score for ranking.
///
/// Returns `None` if `needle` does not match `haystack` as a subsequence.
/// Matching is ASCII-case-insensitive.
pub fn score(needle: &str, haystack: &str) -> Option<i64> {
    let needle = needle.trim();
    if needle.is_empty() {
        return Some(0);
    }

    let needle_chars: Vec<char> = needle.chars().collect();
    let haystack_chars: Vec<char> = haystack.chars().collect();

    if needle_chars.len() > haystack_chars.len() {
        return None;
    }

    let needle_folded: Vec<char> = needle_chars.into_iter().map(fold).collect();
    let haystack_folded: Vec<char> = haystack_chars.iter().copied().map(fold).collect();

    let match_scores = compute_match_scores(&haystack_chars);

    // DP for best score ending at haystack position j for the current needle index i.
    let n = haystack_folded.len();
    let mut prev = vec![NEG_INF; n];
    let mut curr = vec![NEG_INF; n];

    // Initialize for first needle character.
    let first = needle_folded[0];
    for j in 0..n {
        if haystack_folded[j] == first {
            prev[j] = match_scores[j] - PENALTY_LEADING * (j as i64);
        }
    }

    for i in 1..needle_folded.len() {
        curr.fill(NEG_INF);
        let want = needle_folded[i];

        // best_prefix(j) = max_{k<j} prev[k] + PENALTY_GAP*(k+1)
        let mut best_prefix = NEG_INF;

        for j in 0..n {
            if j > 0 {
                let k = j - 1;
                if prev[k] != NEG_INF {
                    best_prefix = best_prefix.max(prev[k] + PENALTY_GAP * ((k + 1) as i64));
                }
            }

            if haystack_folded[j] != want {
                continue;
            }

            let mut best = NEG_INF;

            // Gap transition from any earlier match.
            if best_prefix != NEG_INF {
                best = best.max(best_prefix - PENALTY_GAP * (j as i64));
            }

            // Consecutive transition from j-1.
            if j > 0 && prev[j - 1] != NEG_INF {
                best = best.max(prev[j - 1] + BONUS_CONSECUTIVE);
            }

            if best != NEG_INF {
                curr[j] = match_scores[j] + best;
            }
        }

        std::mem::swap(&mut prev, &mut curr);
    }

    let best = prev.into_iter().max().unwrap_or(NEG_INF);
    if best == NEG_INF { None } else { Some(best) }
}

/// Score multiple whitespace tokens as an AND query.
///
/// Returns `None` if any token does not match.
#[allow(dead_code)]
pub fn score_tokens(tokens: &[String], haystack: &str) -> Option<i64> {
    let mut total = 0i64;
    for tok in tokens {
        let s = score(tok, haystack)?;
        total += s;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_none_when_not_subsequence() {
        assert_eq!(score("abc", "acb"), None);
    }

    #[test]
    fn score_prefers_consecutive_matches() {
        let a = score("abc", "a_b_c").unwrap();
        let b = score("abc", "abc").unwrap();
        assert!(b > a, "expected consecutive match score to be higher");
    }

    #[test]
    fn score_prefers_word_boundary() {
        let a = score("foo", "xfoo").unwrap();
        let b = score("foo", "x foo").unwrap();
        assert!(b > a, "expected boundary match score to be higher");
    }

    #[test]
    fn score_is_ascii_case_insensitive() {
        let a = score("Foo", "foo").unwrap();
        let b = score("foo", "FOO").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn score_tokens_requires_all_tokens() {
        let tokens = vec!["auth".to_string(), "jwt".to_string()];
        assert!(score_tokens(&tokens, "auth with jwt").is_some());
        assert!(score_tokens(&tokens, "auth only").is_none());
    }

    #[test]
    fn score_matches_multiword_title() {
        assert!(score("carola", "Carola Aldo Olivia").is_some());
        assert!(score("aldo", "Carola Aldo Olivia").is_some());
        assert!(score("olivia", "Carola Aldo Olivia").is_some());
    }
}
