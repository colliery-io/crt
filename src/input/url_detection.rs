//! URL detection in terminal text
//!
//! This module provides pure functions for detecting URLs in terminal text.
//! URLs can be detected for hyperlinking on hover/click.

use regex::Regex;
use std::sync::OnceLock;

/// Get the URL regex (compiled once)
fn url_regex() -> &'static Regex {
    static URL_REGEX: OnceLock<Regex> = OnceLock::new();
    URL_REGEX.get_or_init(|| {
        // Match http://, https://, file:// URLs and www. prefixed domains
        Regex::new(
            r"(?x)
            (?:https?://|file://)  # Protocol
            [^\s<>\[\]{}|\\^`\x00-\x1f]+  # URL characters (no whitespace or special chars)
            |
            (?:www\.)  # www. prefix
            [^\s<>\[\]{}|\\^`\x00-\x1f]+  # URL characters
            ",
        )
        .expect("Invalid URL regex")
    })
}

/// A detected URL with its position in the text
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlMatch {
    /// The URL string as it appears in the text
    pub url: String,
    /// Starting column (0-indexed, byte offset)
    pub start_col: usize,
    /// Ending column (exclusive, 0-indexed, byte offset)
    pub end_col: usize,
}

impl UrlMatch {
    /// Get the full URL with protocol
    ///
    /// Adds `https://` prefix to `www.` URLs that don't have a protocol.
    pub fn full_url(&self) -> String {
        if self.url.starts_with("www.") {
            format!("https://{}", self.url)
        } else {
            self.url.clone()
        }
    }

    /// Check if a column position is within this URL
    pub fn contains_column(&self, col: usize) -> bool {
        col >= self.start_col && col < self.end_col
    }

    /// Get the length of the URL in bytes
    pub fn len(&self) -> usize {
        self.end_col - self.start_col
    }

    /// Check if the URL is empty (should never happen in practice)
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Detect all URLs in a line of text
///
/// Returns a list of URL matches with their positions. The positions are
/// byte offsets into the string.
///
/// # Arguments
/// * `text` - The line of text to search for URLs
///
/// # Examples
/// ```ignore
/// let urls = detect_urls("Visit https://example.com for more");
/// assert_eq!(urls.len(), 1);
/// assert_eq!(urls[0].url, "https://example.com");
/// ```
pub fn detect_urls(text: &str) -> Vec<UrlMatch> {
    let regex = url_regex();
    regex
        .find_iter(text)
        .map(|m| {
            let raw_url = m.as_str();
            // Trim trailing punctuation that's likely not part of the URL
            let url = raw_url.trim_end_matches(|c| matches!(c, '.' | ',' | ')' | ']' | '}' | ';' | ':' | '!' | '?'));
            UrlMatch {
                url: url.to_string(),
                start_col: m.start(),
                end_col: m.start() + url.len(),
            }
        })
        .collect()
}

/// Find the URL at a specific column position
///
/// Returns a reference to the URL match if the column falls within a URL.
///
/// # Arguments
/// * `urls` - The list of detected URLs
/// * `col` - The column position to check (0-indexed byte offset)
pub fn url_at_column<'a>(urls: &'a [UrlMatch], col: usize) -> Option<&'a UrlMatch> {
    urls.iter().find(|u| u.contains_column(col))
}

/// Find the index of the URL at a specific column position
///
/// Returns the index into the `urls` slice if the column falls within a URL.
///
/// # Arguments
/// * `urls` - The list of detected URLs
/// * `col` - The column position to check (0-indexed byte offset)
pub fn url_index_at_column(urls: &[UrlMatch], col: usize) -> Option<usize> {
    urls.iter().position(|u| u.contains_column(col))
}

/// Detect URLs and return the one at the given column, if any
///
/// This is a convenience function that combines `detect_urls` and `url_at_column`.
///
/// # Arguments
/// * `text` - The line of text to search for URLs
/// * `col` - The column position to check (0-indexed byte offset)
pub fn detect_url_at_column(text: &str, col: usize) -> Option<UrlMatch> {
    let urls = detect_urls(text);
    url_at_column(&urls, col).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // === detect_urls tests ===

    #[test]
    fn test_detect_https_url() {
        let urls = detect_urls("Visit https://example.com for more");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
        assert_eq!(urls[0].start_col, 6);
        assert_eq!(urls[0].end_col, 25); // "https://example.com" is 19 chars, 6+19=25
    }

    #[test]
    fn test_detect_http_url() {
        let urls = detect_urls("Go to http://example.org/path");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "http://example.org/path");
    }

    #[test]
    fn test_detect_www_url() {
        let urls = detect_urls("Go to www.example.com");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "www.example.com");
        assert_eq!(urls[0].full_url(), "https://www.example.com");
    }

    #[test]
    fn test_detect_file_url() {
        let urls = detect_urls("Open file:///path/to/file.txt");
        assert_eq!(urls.len(), 1);
        assert!(urls[0].url.starts_with("file://"));
    }

    #[test]
    fn test_detect_url_with_path() {
        let urls = detect_urls("See https://example.com/path/to/page?query=1&other=2");
        assert_eq!(urls.len(), 1);
        assert!(urls[0].url.contains("path/to/page"));
        assert!(urls[0].url.contains("query=1"));
    }

    #[test]
    fn test_detect_multiple_urls() {
        let urls = detect_urls("See https://a.com and https://b.com for info");
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].url, "https://a.com");
        assert_eq!(urls[1].url, "https://b.com");
    }

    #[test]
    fn test_no_urls() {
        let urls = detect_urls("No URLs here, just plain text");
        assert!(urls.is_empty());
    }

    #[test]
    fn test_url_at_start() {
        let urls = detect_urls("https://example.com is the site");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].start_col, 0);
    }

    #[test]
    fn test_url_at_end() {
        let urls = detect_urls("Visit https://example.com");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].end_col, 25);
    }

    // === Trailing punctuation trimming ===

    #[test]
    fn test_trim_trailing_period() {
        let urls = detect_urls("See https://example.com.");
        assert_eq!(urls[0].url, "https://example.com");
    }

    #[test]
    fn test_trim_trailing_comma() {
        let urls = detect_urls("https://example.com, and more");
        assert_eq!(urls[0].url, "https://example.com");
    }

    #[test]
    fn test_trim_trailing_paren() {
        let urls = detect_urls("(see https://example.com)");
        assert_eq!(urls[0].url, "https://example.com");
    }

    #[test]
    fn test_trim_trailing_bracket() {
        let urls = detect_urls("[https://example.com]");
        assert_eq!(urls[0].url, "https://example.com");
    }

    #[test]
    fn test_trim_trailing_exclamation() {
        let urls = detect_urls("Check https://example.com!");
        assert_eq!(urls[0].url, "https://example.com");
    }

    #[test]
    fn test_trim_trailing_question() {
        let urls = detect_urls("Is it https://example.com?");
        assert_eq!(urls[0].url, "https://example.com");
    }

    // === url_at_column tests ===

    #[test]
    fn test_url_at_column_start() {
        let urls = detect_urls("Visit https://example.com here");
        assert!(url_at_column(&urls, 6).is_some());
    }

    #[test]
    fn test_url_at_column_middle() {
        let urls = detect_urls("Visit https://example.com here");
        assert!(url_at_column(&urls, 15).is_some());
    }

    #[test]
    fn test_url_at_column_end() {
        let urls = detect_urls("Visit https://example.com here");
        // Column 24 is the last column within the URL (end_col 25 is exclusive)
        assert!(url_at_column(&urls, 24).is_some());
        // Column 25 is past the end
        assert!(url_at_column(&urls, 25).is_none());
    }

    #[test]
    fn test_url_at_column_before() {
        let urls = detect_urls("Visit https://example.com here");
        assert!(url_at_column(&urls, 0).is_none());
        assert!(url_at_column(&urls, 5).is_none());
    }

    #[test]
    fn test_url_at_column_after() {
        let urls = detect_urls("Visit https://example.com here");
        assert!(url_at_column(&urls, 27).is_none());
        assert!(url_at_column(&urls, 30).is_none());
    }

    #[test]
    fn test_url_at_column_empty_list() {
        let urls: Vec<UrlMatch> = vec![];
        assert!(url_at_column(&urls, 5).is_none());
    }

    // === url_index_at_column tests ===

    #[test]
    fn test_url_index_at_column() {
        let urls = detect_urls("https://a.com and https://b.com");
        assert_eq!(url_index_at_column(&urls, 5), Some(0));
        assert_eq!(url_index_at_column(&urls, 20), Some(1));
        assert_eq!(url_index_at_column(&urls, 15), None);
    }

    // === detect_url_at_column tests ===

    #[test]
    fn test_detect_url_at_column_found() {
        let url = detect_url_at_column("Visit https://example.com", 10);
        assert!(url.is_some());
        assert_eq!(url.unwrap().url, "https://example.com");
    }

    #[test]
    fn test_detect_url_at_column_not_found() {
        let url = detect_url_at_column("Visit https://example.com", 0);
        assert!(url.is_none());
    }

    // === UrlMatch methods ===

    #[test]
    fn test_url_match_full_url_with_protocol() {
        let url = UrlMatch {
            url: "https://example.com".to_string(),
            start_col: 0,
            end_col: 19,
        };
        assert_eq!(url.full_url(), "https://example.com");
    }

    #[test]
    fn test_url_match_full_url_www() {
        let url = UrlMatch {
            url: "www.example.com".to_string(),
            start_col: 0,
            end_col: 15,
        };
        assert_eq!(url.full_url(), "https://www.example.com");
    }

    #[test]
    fn test_url_match_len() {
        let url = UrlMatch {
            url: "https://example.com".to_string(),
            start_col: 5,
            end_col: 24,
        };
        assert_eq!(url.len(), 19);
    }

    #[test]
    fn test_url_match_contains_column() {
        let url = UrlMatch {
            url: "https://example.com".to_string(),
            start_col: 5,
            end_col: 24,
        };
        assert!(!url.contains_column(4));
        assert!(url.contains_column(5));
        assert!(url.contains_column(15));
        assert!(url.contains_column(23));
        assert!(!url.contains_column(24));
    }
}
