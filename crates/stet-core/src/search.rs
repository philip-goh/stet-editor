//! Literal search over a [`TextBuffer`].
//!
//! This is the model behind the find bar: it turns a query into a sorted
//! list of byte ranges with no UI concerns, so it is fully unit-testable.
//! v0.1 supports literal, single-line queries only; regex, whole-word and
//! multi-line support are roadmap items.

use std::ops::Range;

use crate::text::TextBuffer;

/// How many matches a search will collect before giving up.
///
/// The cap keeps a pathological query (say, `e` in a 10 MiB log) from
/// freezing the UI. The UI reports truncation to the user.
pub const MAX_MATCHES: usize = 10_000;

/// Search configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    /// Match case exactly.
    pub case_sensitive: bool,
}

/// The result of a search: sorted, non-overlapping byte ranges.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchResult {
    /// Matches in document order.
    pub matches: Vec<Range<usize>>,
    /// Whether the match list was cut off at [`MAX_MATCHES`].
    pub truncated: bool,
}

impl SearchResult {
    /// Returns the number of matches found.
    pub fn len(&self) -> usize {
        self.matches.len()
    }

    /// Returns `true` if there are no matches.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Returns the first match at or after `cursor`, wrapping around.
    ///
    /// A match containing `cursor` counts as "at" the cursor.
    pub fn next_from(&self, cursor: usize) -> Option<&Range<usize>> {
        self.matches
            .iter()
            .find(|range| range.end > cursor)
            .or_else(|| self.matches.first())
    }

    /// Returns the index of the first match at or after `cursor`, wrapping.
    pub fn next_index_from(&self, cursor: usize) -> Option<usize> {
        let index = self
            .matches
            .iter()
            .position(|range| range.end > cursor)
            .unwrap_or(0);
        (!self.matches.is_empty()).then_some(index)
    }

    /// Returns the last match that starts before `cursor`, wrapping around.
    pub fn previous_index_from(&self, cursor: usize) -> Option<usize> {
        let index = self
            .matches
            .iter()
            .rposition(|range| range.start < cursor)
            .unwrap_or_else(|| self.matches.len().saturating_sub(1));
        (!self.matches.is_empty()).then_some(index)
    }
}

/// Finds all matches of `needle` in `buffer`.
///
/// Queries that are empty or contain a line break produce no matches; the
/// latter is a documented v0.1 limitation (see the module docs).
pub fn find_matches(buffer: &TextBuffer, needle: &str, options: SearchOptions) -> SearchResult {
    if needle.is_empty() || needle.contains('\n') || needle.contains('\r') {
        return SearchResult::default();
    }

    let mut matches = Vec::new();
    let mut truncated = false;

    for line in 0..buffer.line_count() {
        if matches.len() >= MAX_MATCHES {
            truncated = true;
            break;
        }
        let content = buffer.line_content(line);
        let line_start = buffer.line_start_offset(line);
        for range in find_in_line(&content, needle, options) {
            matches.push(line_start + range.start..line_start + range.end);
            if matches.len() >= MAX_MATCHES {
                truncated = true;
                break;
            }
        }
    }

    SearchResult { matches, truncated }
}

/// Finds non-overlapping matches of `needle` within one line.
///
/// Matches are returned as line-relative byte ranges. Case-insensitive
/// matching compares per-character lowercase forms, so byte offsets stay
/// valid even for multi-byte text.
fn find_in_line(haystack: &str, needle: &str, options: SearchOptions) -> Vec<Range<usize>> {
    let mut matches = Vec::new();
    if needle.is_empty() {
        return matches;
    }

    if options.case_sensitive {
        let mut search_from = 0;
        while let Some(found) = haystack[search_from..].find(needle) {
            let start = search_from + found;
            let end = start + needle.len();
            matches.push(start..end);
            search_from = end;
        }
        return matches;
    }
    let needle_chars: Vec<char> = needle.chars().collect();
    let mut index = 0;
    while index < haystack.len() {
        if !haystack.is_char_boundary(index) {
            index += 1;
            continue;
        }
        let mut haystack_iter = haystack[index..].char_indices();
        let mut matched = true;
        for needle_char in &needle_chars {
            match haystack_iter.next() {
                Some((_, haystack_char)) if lowercase_eq(haystack_char, *needle_char) => {}
                _ => {
                    matched = false;
                    break;
                }
            }
        }
        if matched {
            let end = match haystack_iter.next() {
                Some((offset, _)) => index + offset,
                None => haystack.len(),
            };
            matches.push(index..end);
            index = end;
        } else {
            index += haystack[index..].chars().next().map_or(1, char::len_utf8);
        }
    }
    matches
}

/// Case-insensitive character equality that stays at one-char granularity.
///
/// This intentionally does not handle multi-character case mappings
/// (e.g. `ß` ⇄ `SS`); doing so would break the offset arithmetic in the
/// line scanner. Documented v0.1 limitation.
fn lowercase_eq(a: char, b: char) -> bool {
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(&b);
    }
    a == b || a.to_lowercase().eq(b.to_lowercase()) || a.to_uppercase().eq(b.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(text: &str) -> TextBuffer {
        TextBuffer::new(text)
    }

    fn ranges(result: &SearchResult) -> Vec<Range<usize>> {
        result.matches.clone()
    }

    #[test]
    fn finds_literal_matches_in_order() {
        let buffer = buffer("one two one\none");
        let result = find_matches(
            &buffer,
            "one",
            SearchOptions {
                case_sensitive: true,
            },
        );
        assert_eq!(ranges(&result), vec![0..3, 8..11, 12..15]);
        assert!(!result.truncated);
    }

    #[test]
    fn does_not_overlap_matches() {
        let buffer = buffer("aaaa");
        let result = find_matches(
            &buffer,
            "aa",
            SearchOptions {
                case_sensitive: true,
            },
        );
        assert_eq!(ranges(&result), vec![0..2, 2..4]);
    }

    #[test]
    fn case_insensitive_by_default() {
        let buffer = buffer("Hello hello HELLO");
        let result = find_matches(&buffer, "hello", SearchOptions::default());
        assert_eq!(ranges(&result), vec![0..5, 6..11, 12..17]);
    }

    #[test]
    fn case_sensitive_respects_case() {
        let buffer = buffer("Hello hello HELLO");
        let result = find_matches(
            &buffer,
            "hello",
            SearchOptions {
                case_sensitive: true,
            },
        );
        assert_eq!(ranges(&result), vec![6..11]);
    }

    #[test]
    fn case_insensitive_matches_multibyte_case_pairs() {
        let buffer = buffer("CAFÉ café");
        let result = find_matches(&buffer, "café", SearchOptions::default());
        assert_eq!(ranges(&result), vec![0..5, 6..11]);
    }

    #[test]
    fn empty_or_multiline_queries_match_nothing() {
        let buffer = buffer("one\ntwo");
        assert!(find_matches(&buffer, "", SearchOptions::default()).is_empty());
        assert!(find_matches(&buffer, "one\ntwo", SearchOptions::default()).is_empty());
    }

    #[test]
    fn next_from_contains_and_wraps() {
        let result = find_matches(
            &buffer("one two one"),
            "one",
            SearchOptions {
                case_sensitive: true,
            },
        );
        assert_eq!(result.next_from(0), Some(&(0..3)));
        assert_eq!(result.next_from(1), Some(&(0..3)));
        assert_eq!(result.next_from(3), Some(&(8..11)));
        assert_eq!(result.next_from(8), Some(&(8..11)));
        assert_eq!(result.next_from(11), Some(&(0..3)));
    }

    #[test]
    fn previous_from_skips_and_wraps() {
        let result = find_matches(
            &buffer("one two one"),
            "one",
            SearchOptions {
                case_sensitive: true,
            },
        );
        assert_eq!(result.previous_index_from(8), Some(0));
        assert_eq!(result.previous_index_from(11), Some(1));
        assert_eq!(result.previous_index_from(0), Some(1));
        assert_eq!(result.previous_index_from(99), Some(1));
    }

    #[test]
    fn empty_result_navigation_returns_none() {
        let result = find_matches(&buffer("abc"), "zzz", SearchOptions::default());
        assert!(result.is_empty());
        assert_eq!(result.next_from(0), None);
        assert_eq!(result.next_index_from(0), None);
        assert_eq!(result.previous_index_from(0), None);
    }
}
