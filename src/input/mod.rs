//! Input handling
//!
//! Keyboard and mouse input processing for terminal and tab bar.

pub mod drag;
mod key_encoder;
mod keyboard;
mod mouse;

pub use key_encoder::encode_key;
pub use keyboard::{KeyboardAction, handle_keyboard_input};
pub use mouse::{
    GridLayout, MouseClickTarget, compute_click_count, determine_click_target,
    handle_cursor_moved, handle_mouse_input, handle_mouse_wheel, normalize_scroll_delta,
    screen_to_grid_position,
};

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crt_core::{Column, Line, Point, SelectionType, ShellTerminal, TermMode};
use regex::Regex;
use winit::keyboard::{Key, NamedKey};

use crate::window::WindowState;

/// Detected URL with its position in the terminal (supports multi-line spans)
#[derive(Debug, Clone)]
pub struct DetectedUrl {
    /// The URL string
    pub url: String,
    /// Starting column on start line (0-indexed)
    pub start_col: usize,
    /// Ending column on end line (exclusive, 0-indexed)
    pub end_col: usize,
    /// Start line number (0-indexed viewport line)
    pub line: usize,
    /// End line number (same as line for single-line URLs)
    pub end_line: usize,
}

/// Get the URL regex (compiled once)
fn url_regex() -> &'static Regex {
    static URL_REGEX: OnceLock<Regex> = OnceLock::new();
    URL_REGEX.get_or_init(|| {
        // Match http://, https://, file:// URLs
        // Also match bare domains like github.com/path
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

/// Trailing characters that read as prose punctuation rather than part of a URL
/// (the period ending a sentence, a comma in a list, etc.).
const URL_TRAILING_PUNCTUATION: &[char] = &['.', ',', ';', ':', '!', '?', '\'', '"', '*'];

/// Strip trailing punctuation that the greedy match swallowed from the
/// surrounding text.
///
/// The regex matches URL characters up to the next whitespace, so a link
/// written in a sentence (`see https://example.com.`) or wrapped in parens
/// (`(https://example.com)`) drags the trailing `.`/`)` into the match, and the
/// browser then fails to open it. We trim those, but keep a closing paren that
/// balances an opening one inside the URL so links like
/// `https://en.wikipedia.org/wiki/Rust_(programming_language)` stay intact.
/// (Only `)` matters here — the regex already excludes `[]{}<>`.)
fn trim_url_trailing_punctuation(url: &str) -> &str {
    let mut end = url.len();
    while let Some(ch) = url[..end].chars().next_back() {
        let strip = if URL_TRAILING_PUNCTUATION.contains(&ch) {
            true
        } else if ch == ')' {
            // Drop a closing paren only when it's unbalanced (more `)` than `(`).
            let span = &url[..end];
            span.matches(')').count() > span.matches('(').count()
        } else {
            false
        };
        if !strip {
            break;
        }
        end -= ch.len_utf8();
    }
    &url[..end]
}

/// Scan a line of text for URLs and return their positions
pub fn detect_urls_in_line(line_text: &str, line_num: usize) -> Vec<DetectedUrl> {
    let regex = url_regex();
    regex
        .find_iter(line_text)
        .map(|m| {
            // Convert byte offsets to character indices for grid column comparison
            let matched = trim_url_trailing_punctuation(m.as_str());
            let start_col = line_text[..m.start()].chars().count();
            let end_col = start_col + matched.chars().count();
            DetectedUrl {
                url: matched.to_string(),
                start_col,
                end_col,
                line: line_num,
                end_line: line_num, // Same line initially, may be extended by merge_wrapped_urls
            }
        })
        .collect()
}

/// Check if a position (col, line) is within a detected URL (supports multi-line URLs)
pub fn find_url_at_position(urls: &[DetectedUrl], col: usize, line: usize) -> Option<&DetectedUrl> {
    urls.iter().find(|url| is_position_in_url(url, col, line))
}

/// Find the index of a URL at a given position (supports multi-line URLs)
pub fn find_url_index_at_position(urls: &[DetectedUrl], col: usize, line: usize) -> Option<usize> {
    urls.iter().position(|url| is_position_in_url(url, col, line))
}

/// Check if a (col, line) position falls within a URL's span
fn is_position_in_url(url: &DetectedUrl, col: usize, line: usize) -> bool {
    is_position_in_span(url.start_col, url.end_col, url.line, url.end_line, col, line)
}

/// Check if a (col, line) position falls within a (possibly multi-line) span.
///
/// Shared by URL and file-path hit testing (and the render layer's hover
/// underline). `start_col`/`end_col` are the 0-indexed inclusive-start /
/// exclusive-end columns on the start/end lines.
pub fn is_position_in_span(
    start_col: usize,
    end_col: usize,
    start_line: usize,
    end_line: usize,
    col: usize,
    line: usize,
) -> bool {
    if line < start_line || line > end_line {
        return false;
    }
    if start_line == end_line {
        // Single-line span
        col >= start_col && col < end_col
    } else if line == start_line {
        // First line of a multi-line span
        col >= start_col
    } else if line == end_line {
        // Last line of a multi-line span
        col < end_col
    } else {
        // Middle line - entirely within the span
        true
    }
}

/// Merge URLs that wrap across multiple lines
///
/// When a URL ends at the last column of a line and the next line continues
/// with URL-like content (no protocol, no leading whitespace), merge them
/// into a single multi-line URL.
pub fn merge_wrapped_urls(urls: &mut Vec<DetectedUrl>, line_texts: &BTreeMap<i32, String>, cols: usize) {
    let mut i = 0;
    while i < urls.len() {
        // Check if URL ends at or near the last column (allowing for slight variance)
        if urls[i].end_col >= cols.saturating_sub(1) {
            let next_line = urls[i].end_line as i32 + 1;
            if let Some(next_text) = line_texts.get(&next_line) {
                // Check if next line could be a URL continuation:
                // - Not empty
                // - Doesn't start with a new protocol (would be a separate URL)
                // - No leading whitespace (hard line break would have content at col 0)
                if !next_text.is_empty()
                    && !next_text.starts_with("http://")
                    && !next_text.starts_with("https://")
                    && !next_text.starts_with("file://")
                    && !next_text.starts_with("www.")
                    && !next_text.starts_with(' ')
                    && !next_text.starts_with('\t')
                {
                    // Find where URL-like characters end on next line
                    let continuation_end = find_url_continuation_end(next_text);
                    if continuation_end > 0 {
                        // Merge: extend URL string and update end position
                        urls[i].url.push_str(&next_text[..continuation_end]);
                        urls[i].end_line = next_line as usize;
                        urls[i].end_col = next_text[..continuation_end].chars().count();
                        // Continue checking - this merged URL might also wrap
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    // Trailing punctuation may now sit at the end of a merged continuation line,
    // so re-trim each final URL and fix up end_col on its last line.
    for url in urls.iter_mut() {
        let new_len = trim_url_trailing_punctuation(&url.url).len();
        let removed = url.url[new_len..].chars().count();
        if removed > 0 {
            url.url.truncate(new_len);
            url.end_col = url.end_col.saturating_sub(removed);
        }
    }
}

/// Find where URL-like characters end in a continuation line
fn find_url_continuation_end(text: &str) -> usize {
    // URL characters: anything except whitespace and certain special chars
    // Match the same pattern as the URL regex
    let mut end = 0;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() || "<>[]{}|\\^`".contains(c) || c < ' ' {
            break;
        }
        end = i + c.len_utf8();
    }
    end
}

/// Detected file path with its position in the terminal grid.
///
/// Parallel to [`DetectedUrl`] but for local filesystem paths. Detection (the
/// producer of this struct) is pure and does no I/O: `exists` is always `false`
/// from detection and is set later by resolution/validation (CRT-T-0203). The
/// column span covers the whole clickable token *including* any `:line[:col]`
/// suffix, while [`DetectedPath::path`] holds only the path portion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedPath {
    /// The path string, with any trailing `:line[:col]` suffix removed.
    pub path: String,
    /// 1-based line number parsed from a trailing `:N` / `:N:M` suffix.
    pub target_line: Option<usize>,
    /// 1-based column parsed from a trailing `:N:M` suffix.
    pub target_col: Option<usize>,
    /// Whether the resolved path exists on disk. Always `false` from detection;
    /// set by resolution/validation (see CRT-T-0203).
    pub exists: bool,
    /// Starting column on the start line (0-indexed).
    pub start_col: usize,
    /// Ending column (exclusive, 0-indexed) on the end line.
    pub end_col: usize,
    /// Start line number (0-indexed viewport line).
    pub line: usize,
    /// End line number (same as `line` for single-line paths).
    pub end_line: usize,
}

/// Get the file-path candidate regex (compiled once).
fn path_regex() -> &'static Regex {
    static PATH_REGEX: OnceLock<Regex> = OnceLock::new();
    PATH_REGEX.get_or_init(|| {
        // Three candidate forms (tried in order at each position):
        //   1. a double-quoted run  "…"   — may contain spaces
        //   2. a single-quoted run  '…'   — may contain spaces
        //   3. a bare run of path characters, where a backslash-escaped space
        //      (`\ `) is also allowed so shell-style escaped paths stay whole.
        // Non-path candidates (no `/`, or URLs) are filtered in
        // `detect_paths_in_line`; on-disk existence is the final gate.
        Regex::new(r#""[^"\n]*"|'[^'\n]*'|(?:[A-Za-z0-9._/~+@%#:-]|\\ )+"#)
            .expect("Invalid path regex")
    })
}

/// Strip one matching pair of surrounding quotes, returning the inner text.
/// Returns `None` if the token isn't quoted.
fn unquote(token: &str) -> Option<&str> {
    let bytes = token.as_bytes();
    if bytes.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && bytes[bytes.len() - 1] == bytes[0]
    {
        // Quotes are ASCII, so these byte indices are char boundaries.
        Some(&token[1..token.len() - 1])
    } else {
        None
    }
}

/// Split a trailing `:line` or `:line:col` suffix off a path token.
///
/// Only a *trailing* run of `:<digits>` (optionally twice) counts, so a `:` that
/// is part of a filename with non-numeric neighbours is left in the path. Returns
/// `(path, line, col)` with 1-based line/col when present.
fn parse_path_suffix(token: &str) -> (&str, Option<usize>, Option<usize>) {
    if let Some((rest, last)) = token.rsplit_once(':')
        && let Ok(last_num) = last.parse::<usize>()
    {
        // `last` is numeric: either `path:line` or `path:line:col`.
        if let Some((rest2, mid)) = rest.rsplit_once(':')
            && let Ok(mid_num) = mid.parse::<usize>()
        {
            return (rest2, Some(mid_num), Some(last_num));
        }
        return (rest, Some(last_num), None);
    }
    (token, None, None)
}

/// Whether a token (suffix already stripped) looks like a path worth offering.
///
/// Requires at least one `/` so bare single words (`README`) are excluded; the
/// on-disk existence check (CRT-T-0203) is the backstop against false positives
/// like `and/or`.
fn is_path_like(path: &str) -> bool {
    path.contains('/')
}

/// Scan a line of text for file-path candidates and return their positions.
///
/// Pure: performs no filesystem access. `exists` is left `false`; callers
/// resolve and validate paths separately.
pub fn detect_paths_in_line(line_text: &str, line_num: usize) -> Vec<DetectedPath> {
    let regex = path_regex();
    let mut paths = Vec::new();
    for m in regex.find_iter(line_text) {
        let raw = m.as_str();

        // The on-screen token (what gets underlined) and the logical path text
        // (used for resolution) differ for quoted / escaped paths.
        let (on_screen, candidate) = if let Some(inner) = unquote(raw) {
            // Quoted: the quotes delimit exactly; spaces inside are literal.
            (raw, inner.to_string())
        } else {
            // Bare: trim trailing prose punctuation, then unescape `\ ` → ` `.
            let trimmed = trim_url_trailing_punctuation(raw);
            (trimmed, trimmed.replace("\\ ", " "))
        };

        // URLs (including `file://`) are the URL detector's job.
        if candidate.contains("://") || on_screen.is_empty() {
            continue;
        }

        let (path_str, target_line, target_col) = parse_path_suffix(&candidate);
        if !is_path_like(path_str) {
            continue;
        }

        let start_col = line_text[..m.start()].chars().count();
        let end_col = start_col + on_screen.chars().count();
        paths.push(DetectedPath {
            path: path_str.to_string(),
            target_line,
            target_col,
            exists: false,
            start_col,
            end_col,
            line: line_num,
            end_line: line_num,
        });
    }
    paths
}

/// Find the path at a given position (supports multi-line paths).
pub fn find_path_at_position(
    paths: &[DetectedPath],
    col: usize,
    line: usize,
) -> Option<&DetectedPath> {
    paths.iter().find(|p| is_position_in_path(p, col, line))
}

/// Find the index of the path at a given position (supports multi-line paths).
pub fn find_path_index_at_position(
    paths: &[DetectedPath],
    col: usize,
    line: usize,
) -> Option<usize> {
    paths.iter().position(|p| is_position_in_path(p, col, line))
}

/// Check if a (col, line) position falls within a path's span.
fn is_position_in_path(path: &DetectedPath, col: usize, line: usize) -> bool {
    is_position_in_span(path.start_col, path.end_col, path.line, path.end_line, col, line)
}

/// Resolve a detected path token to an absolute [`PathBuf`].
///
/// - `~` / `~/...` expand against `home` (returns `None` if `home` is unknown
///   or the token is a `~user` form we don't expand).
/// - Absolute tokens (`/...`) are used as-is.
/// - Anything else is treated as relative and joined onto `cwd` (returns `None`
///   if `cwd` is unknown).
///
/// Pure: no filesystem access. The caller decides existence.
pub fn resolve_path(token: &str, cwd: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    if token == "~" {
        return home.map(Path::to_path_buf);
    }
    if let Some(rest) = token.strip_prefix("~/") {
        return home.map(|h| h.join(rest));
    }
    if token.starts_with('~') {
        // `~user/...` — not supported.
        return None;
    }
    if token.starts_with('/') {
        return Some(PathBuf::from(token));
    }
    cwd.map(|c| c.join(token))
}

/// Maximum number of filesystem existence checks performed in a single
/// validation pass (one frame). Beyond this, further candidates are treated as
/// non-existent rather than stalling the render loop. See NFR-001 (CRT-I-0033).
const MAX_STAT_PER_PASS: usize = 256;

/// Validates detected paths against the filesystem, caching results so a frame
/// does not re-`stat` paths it has already seen.
///
/// The cache is keyed by the *resolved* absolute path and persists across frames
/// while the working directory is unchanged; it is cleared when the cwd changes
/// (relative tokens would otherwise resolve differently). A per-pass budget
/// (`MAX_STAT_PER_PASS`) bounds the number of `stat` calls per frame.
#[derive(Debug, Default)]
pub struct PathValidator {
    cwd: Option<PathBuf>,
    home: Option<PathBuf>,
    cache: HashMap<PathBuf, bool>,
    stats_this_pass: usize,
    budget_warned: bool,
}

impl PathValidator {
    /// Begin a validation pass with the given resolution context. Clears the
    /// existence cache if the working directory changed, and resets the per-pass
    /// `stat` budget.
    pub fn begin_pass(&mut self, cwd: Option<PathBuf>, home: Option<PathBuf>) {
        if self.cwd != cwd {
            self.cache.clear();
            self.cwd = cwd;
        }
        self.home = home;
        self.stats_this_pass = 0;
        self.budget_warned = false;
    }

    /// Resolve and validate a single path, setting [`DetectedPath::exists`].
    pub fn validate(&mut self, path: &mut DetectedPath) {
        let Some(resolved) =
            resolve_path(&path.path, self.cwd.as_deref(), self.home.as_deref())
        else {
            path.exists = false;
            return;
        };

        if let Some(&exists) = self.cache.get(&resolved) {
            path.exists = exists;
            return;
        }

        if self.stats_this_pass >= MAX_STAT_PER_PASS {
            if !self.budget_warned {
                log::warn!(
                    "Path existence-check budget ({}) reached this frame; \
                     remaining candidates treated as non-existent",
                    MAX_STAT_PER_PASS
                );
                self.budget_warned = true;
            }
            path.exists = false;
            return;
        }

        self.stats_this_pass += 1;
        let exists = resolved.try_exists().unwrap_or(false);
        self.cache.insert(resolved, exists);
        path.exists = exists;
    }

    /// Resolve and validate every path in `paths`.
    pub fn validate_all(&mut self, paths: &mut [DetectedPath]) {
        for path in paths.iter_mut() {
            self.validate(path);
        }
    }
}

/// Open a URL in the default browser
pub fn open_url(url: &str) {
    // Ensure URL has a protocol
    let full_url = if url.starts_with("www.") {
        format!("https://{}", url)
    } else {
        url.to_string()
    };

    if let Err(e) = open::that(&full_url) {
        log::error!("Failed to open URL '{}': {}", full_url, e);
    }
}

/// Build the program + args for an editor `open_file_command` template.
///
/// The template is split on whitespace; in each token the placeholders `{file}`,
/// `{line}`, `{col}` are substituted (missing line/col default to `1`). Because
/// substitution happens after splitting, a `{file}` token expands to a single
/// argument even when the path contains spaces. Returns `None` for an
/// empty/whitespace-only template.
fn build_open_command(
    template: &str,
    file: &str,
    line: Option<usize>,
    col: Option<usize>,
) -> Option<(String, Vec<String>)> {
    let line_s = line.unwrap_or(1).to_string();
    let col_s = col.unwrap_or(1).to_string();
    let mut parts = template.split_whitespace().map(|tok| {
        tok.replace("{file}", file)
            .replace("{line}", &line_s)
            .replace("{col}", &col_s)
    });
    let program = parts.next()?;
    let args = parts.collect();
    Some((program, args))
}

/// Open a file path.
///
/// If `command_template` is set (the `open_file_command` config), the file is
/// opened with that command (with `{file}`/`{line}`/`{col}` substituted);
/// otherwise the OS default application is used and line/col are ignored.
pub fn open_file(
    path: &Path,
    line: Option<usize>,
    col: Option<usize>,
    command_template: Option<&str>,
) {
    let file = path.to_string_lossy();
    if let Some(template) = command_template
        && let Some((program, args)) = build_open_command(template, file.as_ref(), line, col)
    {
        if let Err(e) = std::process::Command::new(&program).args(&args).spawn() {
            log::error!(
                "Failed to open file '{}' with command '{}': {}",
                file,
                template,
                e
            );
        }
        return;
    }

    if let Err(e) = open::that(path) {
        log::error!("Failed to open file '{}': {}", file, e);
    }
}

/// Threshold for multi-click detection
const MULTI_CLICK_THRESHOLD: Duration = Duration::from_millis(400);
/// Maximum distance (in cells) for multi-click to register
const MULTI_CLICK_DISTANCE: usize = 1;

// Mouse button codes for mouse reporting protocol
pub const MOUSE_BUTTON_LEFT: u8 = 0;
pub const MOUSE_BUTTON_MIDDLE: u8 = 1;
pub const MOUSE_BUTTON_RIGHT: u8 = 2;
pub const MOUSE_BUTTON_RELEASE: u8 = 3;
pub const MOUSE_BUTTON_MOTION: u8 = 32;
pub const MOUSE_BUTTON_SCROLL_UP: u8 = 64;
pub const MOUSE_BUTTON_SCROLL_DOWN: u8 = 65;

/// Check if the terminal has mouse reporting enabled
pub fn should_report_mouse(shell: &ShellTerminal) -> bool {
    let mode = shell.terminal().inner().mode();
    mode.intersects(
        TermMode::MOUSE_REPORT_CLICK
            | TermMode::MOUSE_DRAG
            | TermMode::MOUSE_MOTION
            | TermMode::SGR_MOUSE,
    )
}

/// Check if the terminal is tracking mouse motion
pub fn should_report_motion(shell: &ShellTerminal, button_pressed: bool) -> bool {
    let mode = shell.terminal().inner().mode();
    // MOUSE_MOTION: report all motion
    // MOUSE_DRAG: report motion only when button is pressed
    mode.contains(TermMode::MOUSE_MOTION) || (mode.contains(TermMode::MOUSE_DRAG) && button_pressed)
}

/// Check if SGR extended mouse mode is enabled
pub fn is_sgr_mouse_mode(shell: &ShellTerminal) -> bool {
    shell
        .terminal()
        .inner()
        .mode()
        .contains(TermMode::SGR_MOUSE)
}

/// Generate mouse escape sequence for terminal
///
/// # Arguments
/// * `button` - Mouse button code (0=left, 1=middle, 2=right, 3=release, 32+=motion, 64/65=scroll)
/// * `col` - Column (0-indexed)
/// * `line` - Line (0-indexed)
/// * `pressed` - Whether this is a press event (for SGR mode)
/// * `sgr_mode` - Whether to use SGR extended encoding
pub fn mouse_report(button: u8, col: usize, line: usize, pressed: bool, sgr_mode: bool) -> Vec<u8> {
    if sgr_mode {
        // SGR extended mode: \x1b[<Btn;Col;RowM (press) or m (release)
        // Uses 1-indexed coordinates
        let suffix = if pressed { 'M' } else { 'm' };
        format!("\x1b[<{};{};{}{}", button, col + 1, line + 1, suffix).into_bytes()
    } else {
        // Legacy X10 mode: \x1b[M<btn+32><col+33><row+33>
        // Coordinates are offset by 32 and limited to 223 (255 - 32)
        let mut seq = vec![0x1b, b'[', b'M'];
        seq.push(button + 32);
        seq.push(((col + 33).min(255)) as u8);
        seq.push(((line + 33).min(255)) as u8);
        seq
    }
}

/// Result of handling tab editing input
pub enum TabEditResult {
    /// Input was handled by tab editing
    Handled,
    /// Input was not handled (not in edit mode or not an edit key)
    NotHandled,
}

/// Handle keyboard input for tab title editing
pub fn handle_tab_editing(state: &mut WindowState, key: &Key, mod_pressed: bool) -> TabEditResult {
    if !state.gpu.tab_bar.is_editing() || mod_pressed {
        return TabEditResult::NotHandled;
    }

    let mut handled = true;
    let mut need_redraw = true;

    match key {
        Key::Named(NamedKey::Enter) => {
            state.gpu.tab_bar.confirm_editing();
        }
        Key::Named(NamedKey::Escape) => {
            state.gpu.tab_bar.cancel_editing();
        }
        Key::Named(NamedKey::Backspace) => {
            state.gpu.tab_bar.edit_backspace();
        }
        Key::Named(NamedKey::Delete) => {
            state.gpu.tab_bar.edit_delete();
        }
        Key::Named(NamedKey::ArrowLeft) => {
            state.gpu.tab_bar.edit_cursor_left();
        }
        Key::Named(NamedKey::ArrowRight) => {
            state.gpu.tab_bar.edit_cursor_right();
        }
        Key::Named(NamedKey::Home) => {
            state.gpu.tab_bar.edit_cursor_home();
        }
        Key::Named(NamedKey::End) => {
            state.gpu.tab_bar.edit_cursor_end();
        }
        Key::Named(NamedKey::Space) => {
            state.gpu.tab_bar.edit_insert_char(' ');
        }
        Key::Character(c) => {
            for ch in c.chars() {
                if !ch.is_control() {
                    state.gpu.tab_bar.edit_insert_char(ch);
                }
            }
        }
        _ => {
            handled = false;
            need_redraw = false;
        }
    }

    if need_redraw {
        state.render.dirty = true;
        state.window.request_redraw();
    }

    if handled {
        TabEditResult::Handled
    } else {
        TabEditResult::NotHandled
    }
}

/// Handle shell input (send to PTY)
///
/// Uses termwiz for key-to-escape-sequence encoding, with platform-specific
/// overrides for macOS word navigation. Falls back to the event's text field
/// for any keys termwiz doesn't handle.
pub fn handle_shell_input(
    state: &mut WindowState,
    key: &Key,
    text: Option<&str>,
    mod_pressed: bool,
    ctrl_pressed: bool,
    shift_pressed: bool,
    alt_pressed: bool,
) -> bool {
    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return false };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return false;
    };

    log::debug!(
        "Shell input: key={:?} text={:?} mod={} ctrl={} shift={} alt={}",
        key,
        text,
        mod_pressed,
        ctrl_pressed,
        shift_pressed,
        alt_pressed
    );

    let mut input_sent = false;

    // Handle Home/End keys explicitly using readline's native bindings
    // Ctrl-A (0x01) = beginning of line, Ctrl-E (0x05) = end of line
    // These work universally in bash, zsh, and other readline-based shells
    match key {
        Key::Named(NamedKey::Home) if !shift_pressed => {
            shell.send_input(b"\x01"); // Ctrl-A = beginning of line
            input_sent = true;
        }
        Key::Named(NamedKey::End) if !shift_pressed => {
            shell.send_input(b"\x05"); // Ctrl-E = end of line
            input_sent = true;
        }
        _ => {}
    }

    // macOS-specific word/line navigation shortcuts (Option+Arrow, Cmd+Arrow, Option+Backspace)
    // These override standard encoding because macOS users expect this behavior
    #[cfg(target_os = "macos")]
    if !input_sent {
        match key {
            Key::Named(NamedKey::Backspace) if alt_pressed => {
                shell.send_input(b"\x1b\x7f"); // ESC DEL = delete word backward
                input_sent = true;
            }
            // Cmd+Arrow = Home/End (same as Home/End keys above)
            // Use readline bindings for universal shell compatibility
            Key::Named(NamedKey::ArrowRight) if mod_pressed && !shift_pressed => {
                shell.send_input(b"\x05"); // Ctrl-E = end of line
                input_sent = true;
            }
            Key::Named(NamedKey::ArrowLeft) if mod_pressed && !shift_pressed => {
                shell.send_input(b"\x01"); // Ctrl-A = beginning of line
                input_sent = true;
            }
            // Option+Arrow = word navigation
            Key::Named(NamedKey::ArrowRight) if alt_pressed => {
                shell.send_input(b"\x1bf"); // ESC f = forward word
                input_sent = true;
            }
            Key::Named(NamedKey::ArrowLeft) if alt_pressed => {
                shell.send_input(b"\x1bb"); // ESC b = backward word
                input_sent = true;
            }
            _ => {}
        }
    }

    // If not handled by platform-specific code, use termwiz encoding
    if !input_sent {
        // Don't send Cmd+key combinations to terminal (they're app shortcuts)
        if !mod_pressed
            && let Some(bytes) = encode_key(key, ctrl_pressed, shift_pressed, alt_pressed)
        {
            shell.send_input(&bytes);
            input_sent = true;
            log::debug!("Sent via termwiz: {:?}", bytes);
        }
    }

    // Final fallback: use the text field from the key event
    // This catches any keys termwiz doesn't handle
    if !input_sent
        && let Some(t) = text
        && !t.is_empty()
        && !mod_pressed
    {
        shell.send_input(t.as_bytes());
        input_sent = true;
        log::debug!("Forwarded via text field: {:?}", t);
    }

    if input_sent {
        // Scroll to bottom when user types (show live output)
        if shell.is_scrolled_back() {
            shell.scroll_to_bottom();
        }
        // Always invalidate content hash when input is sent to ensure re-render
        // even if PTY output hasn't arrived yet (handles TUI apps like Claude Code)
        state.content_hashes.insert(tab_id, 0);
        state.render.dirty = true;
        state.window.request_redraw();
    }

    input_sent
}

/// Handle mouse click on tab bar
pub fn handle_tab_click(state: &mut WindowState, x: f32, y: f32, now: std::time::Instant) -> bool {
    let double_click_threshold = std::time::Duration::from_millis(400);

    let mut tab_closed = None;
    let mut tab_switched = false;
    let mut started_editing = false;

    if state.gpu.tab_bar.is_editing() {
        if let Some(editing_id) = state.gpu.tab_bar.editing_tab_id() {
            if let Some((tab_id, _)) = state.gpu.tab_bar.hit_test(x, y) {
                if tab_id != editing_id {
                    state.gpu.tab_bar.confirm_editing();
                    state.gpu.tab_bar.select_tab(tab_id);
                    tab_switched = true;
                }
            } else {
                state.gpu.tab_bar.confirm_editing();
            }
        }
    } else if let Some((tab_id, is_close)) = state.gpu.tab_bar.hit_test(x, y) {
        if is_close {
            if state.gpu.tab_bar.tab_count() > 1 {
                state.gpu.tab_bar.close_tab(tab_id);
                tab_closed = Some(tab_id);
                tab_switched = true;
            }
        } else {
            let is_double_click = state
                .interaction
                .last_click_time
                .map(|t| now.duration_since(t) < double_click_threshold)
                .unwrap_or(false)
                && state.interaction.last_click_tab == Some(tab_id);

            if is_double_click {
                // Cancel window rename if active
                if state.ui.window_rename.active {
                    state.ui.window_rename.cancel();
                }
                state.gpu.tab_bar.start_editing(tab_id);
                started_editing = true;
                state.interaction.last_click_time = None;
                state.interaction.last_click_tab = None;
            } else {
                state.gpu.tab_bar.select_tab(tab_id);
                tab_switched = true;
                state.interaction.last_click_time = Some(now);
                state.interaction.last_click_tab = Some(tab_id);
            }
        }
    } else {
        state.interaction.last_click_time = None;
        state.interaction.last_click_tab = None;
    }

    if let Some(tab_id) = tab_closed {
        state.remove_shell_for_tab(tab_id);
    }

    if tab_switched || started_editing {
        state.force_active_tab_redraw();
        state.window.request_redraw();
        true
    } else {
        false
    }
}

/// Handle window resize
pub fn handle_resize(
    state: &mut WindowState,
    shared: &crate::gpu::SharedGpuState,
    new_width: u32,
    new_height: u32,
) {
    use crt_core::Size;

    if new_width < 100 || new_height < 80 {
        return;
    }

    let scale_factor = state.scale_factor;
    let cell_width = state.gpu.glyph_cache.cell_width();
    let line_height = state.gpu.glyph_cache.line_height();
    let tab_bar_height = state.gpu.tab_bar.height();

    let padding_physical = 20.0 * scale_factor;
    let tab_bar_physical = tab_bar_height * scale_factor;

    // Tab bar is always at top, so subtract its height from content area
    let content_width = (new_width as f32 - padding_physical).max(60.0);
    let content_height = (new_height as f32 - padding_physical - tab_bar_physical).max(40.0);

    let new_cols = ((content_width / cell_width) as usize).max(10);
    let new_rows = ((content_height / line_height) as usize).max(4);

    state.cols = new_cols;
    state.rows = new_rows;

    // Resize all shells in this window
    for shell in state.shells.values_mut() {
        shell.resize(Size::new(new_cols, new_rows));
    }

    // Update GPU resources
    state.gpu.config.width = new_width;
    state.gpu.config.height = new_height;
    state
        .gpu
        .surface
        .configure(&shared.device, &state.gpu.config);

    // Recreate text texture for glow effect from pool (old texture returns to pool)
    let text_texture =
        match shared
            .texture_pool
            .checkout(new_width, new_height, state.gpu.config.format)
        {
            Some(t) => t,
            None => {
                log::error!(
                    "Failed to checkout texture from pool during resize - skipping texture update"
                );
                return;
            }
        };
    let composite_bind_group = state
        .gpu
        .effect_pipeline
        .composite
        .create_bind_group(&shared.device, text_texture.view());
    state.gpu.text_texture = text_texture;
    state.gpu.composite_bind_group = composite_bind_group;

    state
        .gpu
        .grid_renderer
        .update_screen_size(&shared.queue, new_width as f32, new_height as f32);
    state.gpu.output_grid_renderer.update_screen_size(
        &shared.queue,
        new_width as f32,
        new_height as f32,
    );
    state.gpu.tab_title_renderer.update_screen_size(
        &shared.queue,
        new_width as f32,
        new_height as f32,
    );

    state
        .gpu
        .tab_bar
        .resize(new_width as f32, new_height as f32);

    state.render.dirty = true;
    for hash in state.content_hashes.values_mut() {
        *hash = 0;
    }
    state.window.request_redraw();
}

/// Convert screen coordinates to terminal cell (column, line)
/// Returns None if the position is outside the terminal area
pub fn screen_to_cell(state: &WindowState, x: f32, y: f32) -> Option<(usize, usize)> {
    let (offset_x, offset_y) = state.gpu.tab_bar.content_offset();
    let layout = GridLayout {
        content_offset_x: offset_x,
        content_offset_y: offset_y,
        padding: 10.0 * state.scale_factor,
        cell_width: state.gpu.glyph_cache.cell_width(),
        line_height: state.gpu.glyph_cache.line_height(),
        max_cols: state.cols,
        max_rows: state.rows,
    };
    screen_to_grid_position(x, y, &layout)
}

/// Handle mouse press for terminal selection or mouse reporting
/// Returns true if the press was handled (was in terminal area)
#[allow(dead_code)]
pub fn handle_terminal_mouse_press(state: &mut WindowState, x: f32, y: f32, now: Instant) -> bool {
    handle_terminal_mouse_button(state, x, y, now, MOUSE_BUTTON_LEFT, true)
}

/// Handle mouse button press/release for any button
/// Returns true if the event was handled (was in terminal area)
pub fn handle_terminal_mouse_button(
    state: &mut WindowState,
    x: f32,
    y: f32,
    now: Instant,
    button: u8,
    pressed: bool,
) -> bool {
    // Check if click is in tab bar area first
    let tab_bar_height = state.gpu.tab_bar.height() * state.scale_factor;
    if y < tab_bar_height {
        return false; // Let tab bar handle it
    }

    let Some((col, line)) = screen_to_cell(state, x, y) else {
        return false;
    };

    // Get the active shell
    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return false };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return false;
    };

    // Check if we should report mouse events to the terminal
    if should_report_mouse(shell) {
        let sgr = is_sgr_mouse_mode(shell);
        let report_button = if !pressed && !sgr {
            // In legacy mode, release is button 3
            MOUSE_BUTTON_RELEASE
        } else {
            button
        };
        let seq = mouse_report(report_button, col, line, pressed, sgr);
        shell.send_input(&seq);

        // Track button state for drag reporting
        if button == MOUSE_BUTTON_LEFT {
            state.interaction.mouse_pressed = pressed;
        }

        state.render.dirty = true;
        state.window.request_redraw();
        return true;
    }

    // Local selection handling (mouse mode not enabled)
    if button != MOUSE_BUTTON_LEFT {
        return false; // Only left button for selection
    }

    if pressed {
        // Determine click count for multi-click selection
        let click_count = if let (Some(last_time), Some((last_col, last_line))) = (
            state.interaction.last_selection_click_time,
            state.interaction.last_selection_click_pos,
        ) {
            let time_ok = now.duration_since(last_time) < MULTI_CLICK_THRESHOLD;
            let pos_ok = col.abs_diff(last_col) <= MULTI_CLICK_DISTANCE
                && line.abs_diff(last_line) <= MULTI_CLICK_DISTANCE;

            if time_ok && pos_ok {
                (state.interaction.selection_click_count % 3) + 1
            } else {
                1
            }
        } else {
            1
        };

        state.interaction.selection_click_count = click_count;
        state.interaction.last_selection_click_time = Some(now);
        state.interaction.last_selection_click_pos = Some((col, line));
        state.interaction.mouse_pressed = true;

        // Convert viewport coordinates to grid coordinates
        // Grid coordinates: negative = scrollback history, 0+ = visible screen
        // Viewport coordinates: 0 = top of visible area
        // When scrolled back, display_offset > 0, so grid_line = viewport_line - display_offset
        let display_offset = shell.terminal().display_offset() as i32;
        let grid_line = line as i32 - display_offset;
        let point = Point::new(Line(grid_line), Column(col));

        // Selection type based on click count
        let selection_type = match click_count {
            1 => SelectionType::Simple,
            2 => SelectionType::Semantic, // Word selection
            3 => SelectionType::Lines,    // Line selection
            _ => SelectionType::Simple,
        };

        shell.start_selection(point, selection_type);

        // For semantic and lines selection, also set the end point immediately
        // to show the full word/line
        if click_count > 1 {
            shell.update_selection(point);
        }
    } else {
        state.interaction.mouse_pressed = false;
    }

    state.render.dirty = true;
    state.window.request_redraw();
    true
}

/// Handle mouse move for terminal selection (dragging) or mouse motion reporting
pub fn handle_terminal_mouse_move(state: &mut WindowState, x: f32, y: f32) {
    let Some((col, line)) = screen_to_cell(state, x, y) else {
        return;
    };

    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return;
    };

    // Check if we should report motion to terminal
    if should_report_motion(shell, state.interaction.mouse_pressed) {
        let sgr = is_sgr_mouse_mode(shell);
        // Button code: 32 + button for motion with button, or just 35 for motion without
        let button = if state.interaction.mouse_pressed {
            MOUSE_BUTTON_MOTION + MOUSE_BUTTON_LEFT // 32 = motion with left button
        } else {
            MOUSE_BUTTON_MOTION + MOUSE_BUTTON_RELEASE // 35 = motion without button
        };
        let seq = mouse_report(button, col, line, true, sgr);
        shell.send_input(&seq);

        state.render.dirty = true;
        state.window.request_redraw();
        return;
    }

    // Local selection handling
    if !state.interaction.mouse_pressed {
        return;
    }

    // Convert viewport coordinates to grid coordinates (same as in mouse button handler)
    let display_offset = shell.terminal().display_offset() as i32;
    let grid_line = line as i32 - display_offset;
    let point = Point::new(Line(grid_line), Column(col));
    shell.update_selection(point);

    state.render.dirty = true;
    state.window.request_redraw();
}

/// Handle mouse release for terminal selection or mouse reporting
pub fn handle_terminal_mouse_release(state: &mut WindowState, x: f32, y: f32) {
    let Some((col, line)) = screen_to_cell(state, x, y) else {
        state.interaction.mouse_pressed = false;
        return;
    };

    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else {
        state.interaction.mouse_pressed = false;
        return;
    };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        state.interaction.mouse_pressed = false;
        return;
    };

    // Check if we should report the release to terminal
    if should_report_mouse(shell) {
        let sgr = is_sgr_mouse_mode(shell);
        let button = if sgr {
            MOUSE_BUTTON_LEFT // SGR mode sends button with release suffix 'm'
        } else {
            MOUSE_BUTTON_RELEASE // Legacy mode sends button 3 for release
        };
        let seq = mouse_report(button, col, line, false, sgr);
        shell.send_input(&seq);
    }

    state.interaction.mouse_pressed = false;
    // Selection remains if we were doing local selection - user can copy with Cmd+C
}

/// Handle mouse scroll wheel for terminal scrollback or mouse reporting
/// Returns true if the scroll was handled by mouse reporting
pub fn handle_terminal_scroll(state: &mut WindowState, x: f32, y: f32, delta_y: f32) -> bool {
    let Some((col, line)) = screen_to_cell(state, x, y) else {
        return false;
    };

    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return false };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return false;
    };

    // Check if we should report scroll to terminal
    if should_report_mouse(shell) {
        let sgr = is_sgr_mouse_mode(shell);
        // Scroll up = 64, scroll down = 65
        let button = if delta_y > 0.0 {
            MOUSE_BUTTON_SCROLL_UP
        } else {
            MOUSE_BUTTON_SCROLL_DOWN
        };
        let seq = mouse_report(button, col, line, true, sgr);
        shell.send_input(&seq);

        state.render.dirty = true;
        state.window.request_redraw();
        return true;
    }

    false
}

/// Clear terminal selection (e.g., when user types or presses Escape)
pub fn clear_terminal_selection(state: &mut WindowState) {
    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return;
    };

    if shell.has_selection() {
        shell.clear_selection();
        state.render.dirty = true;
        state.window.request_redraw();
    }
}

/// Get selected text from terminal (for copy)
pub fn get_terminal_selection_text(state: &WindowState) -> Option<String> {
    let tab_id = state.gpu.tab_bar.active_tab_id()?;
    let shell = state.shells.get(&tab_id)?;
    shell.selection_to_string()
}

/// Get clipboard content from system clipboard
///
/// Handles three types of clipboard content:
/// 1. Text - returns the text directly
/// 2. Files (copied from Finder/Explorer) - returns the file path(s)
/// 3. Images (screenshots) - saves to temp file and returns the path
///
/// This allows pasting images into applications like Claude Code that accept file paths.
pub fn get_clipboard_content() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;

    // First try to get text (most common case)
    if let Ok(text) = clipboard.get_text()
        && !text.is_empty()
    {
        return Some(text);
    }

    // Try to get file paths (files copied from Finder/Explorer)
    if let Ok(files) = clipboard_files::read()
        && !files.is_empty()
    {
        // Join multiple paths with spaces, quoting paths that contain spaces
        let paths: Vec<String> = files
            .into_iter()
            .map(|p| {
                let path_str = p.to_string_lossy().to_string();
                if path_str.contains(' ') {
                    format!("'{}'", path_str)
                } else {
                    path_str
                }
            })
            .collect();
        return Some(paths.join(" "));
    }

    // Try to get image data (screenshots, copied images)
    if let Ok(image_data) = clipboard.get_image()
        && let Some(path) = save_clipboard_image_to_temp(&image_data)
    {
        return Some(path);
    }

    None
}

/// Save clipboard image data to a temporary file and return the path
fn save_clipboard_image_to_temp(image_data: &arboard::ImageData) -> Option<String> {
    use image::ImageEncoder;
    use std::io::Write;

    // Create temp directory if it doesn't exist
    let temp_dir = std::env::temp_dir().join("crt_clipboard");
    std::fs::create_dir_all(&temp_dir).ok()?;

    // Generate unique filename with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let filename = format!("clipboard_{}.png", timestamp);
    let path = temp_dir.join(&filename);

    // Convert RGBA bytes to PNG using the image crate
    let img = image::RgbaImage::from_raw(
        image_data.width as u32,
        image_data.height as u32,
        image_data.bytes.to_vec(),
    )?;

    // Save as PNG
    let mut file = std::fs::File::create(&path).ok()?;
    let encoder = image::codecs::png::PngEncoder::new(&mut file);
    encoder
        .write_image(
            &img,
            image_data.width as u32,
            image_data.height as u32,
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    file.flush().ok()?;

    Some(path.to_string_lossy().to_string())
}

/// Set clipboard content
pub fn set_clipboard_content(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text.to_string());
    }
}

/// Paste content to terminal with bracketed paste mode support
///
/// If the terminal has bracketed paste mode enabled, the content will be
/// wrapped with escape sequences to indicate a paste operation.
pub fn paste_to_terminal(state: &mut WindowState, content: &str) {
    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return;
    };

    log::info!("=== PASTE START ===");
    log::info!("Paste content length: {} bytes", content.len());
    // Use chars().take() to safely handle multi-byte UTF-8 characters
    let preview: String = content.chars().take(50).collect();
    log::info!("Paste content preview: {:?}", preview);

    // Check if bracketed paste mode is enabled
    let bracketed = shell.bracketed_paste_enabled();
    log::info!("Bracketed paste mode: {}", bracketed);

    // Log cursor position before paste
    let cursor_before = shell.terminal().cursor();
    log::info!(
        "Cursor before paste: line={}, col={}",
        cursor_before.point.line.0,
        cursor_before.point.column.0
    );

    if bracketed {
        // Bracketed paste mode: wrap with escape sequences
        shell.send_input(b"\x1b[200~");
        shell.send_input(content.as_bytes());
        shell.send_input(b"\x1b[201~");
    } else {
        shell.send_input(content.as_bytes());
    }

    // Scroll to bottom and clear selection
    if shell.is_scrolled_back() {
        shell.scroll_to_bottom();
        log::info!("Scrolled to bottom");
    }
    clear_terminal_selection(state);
    log::info!("Selection cleared");

    // Always invalidate content hash when pasting to ensure re-render
    // even if PTY output hasn't arrived yet (fixes paste rendering artifacts)
    state.content_hashes.insert(tab_id, 0);
    state.render.dirty = true;
    // Mark paste pending so renderer can normalize INVERSE flags
    // (zsh enables INVERSE mid-line for paste highlighting, creating visual discontinuity)
    state.render.paste_pending = true;
    state.window.request_redraw();
    log::info!("=== PASTE END (hash invalidated, paste_pending=true, redraw requested) ===");
}

/// Scroll terminal to make current search match visible
pub fn scroll_to_current_match(state: &mut WindowState) {
    use crt_core::Scroll;

    if state.ui.search.matches.is_empty() {
        return;
    }

    let current_match = &state.ui.search.matches[state.ui.search.current_match];
    let match_line = current_match.line;

    // Get active shell
    let active_tab_id = state.gpu.tab_bar.active_tab_id();
    let shell = active_tab_id.and_then(|id| state.shells.get_mut(&id));
    let Some(shell) = shell else { return };

    let terminal = shell.terminal();
    let screen_lines = terminal.screen_lines() as i32;
    let display_offset = terminal.display_offset() as i32;

    // Calculate viewport line (what line would the match be at in current viewport)
    // viewport_line = grid_line + display_offset
    let viewport_line = match_line + display_offset;

    // If match is outside visible range (0 to screen_lines-1), scroll to center it
    if viewport_line < 0 || viewport_line >= screen_lines {
        // Target: put match roughly in the middle of the screen
        let target_viewport_line = screen_lines / 2;
        // New display_offset needed: match_line + new_offset = target_viewport_line
        // new_offset = target_viewport_line - match_line
        let new_offset = target_viewport_line - match_line;

        // The scroll delta is the change in display_offset
        // Positive delta scrolls up (increases display_offset)
        let scroll_delta = new_offset - display_offset;

        if scroll_delta != 0 {
            shell.scroll(Scroll::Delta(scroll_delta));
            if let Some(tab_id) = active_tab_id {
                state.content_hashes.insert(tab_id, 0);
            }
        }
    }
}

/// Update search matches based on current query
pub fn update_search_matches(state: &mut WindowState) {
    use crate::window::SearchMatch;

    state.ui.search.matches.clear();
    state.ui.search.current_match = 0;

    let query = &state.ui.search.query;
    if query.is_empty() {
        return;
    }

    // Get active shell's terminal content
    let active_tab_id = state.gpu.tab_bar.active_tab_id();
    let shell = active_tab_id.and_then(|id| state.shells.get(&id));
    let Some(shell) = shell else { return };

    let terminal = shell.terminal();

    // Get all lines including history
    let all_lines = terminal.all_lines_text();

    // Search each line for the query (case-insensitive)
    let query_lower = query.to_lowercase();
    for (line_idx, line_text) in &all_lines {
        let line_lower = line_text.to_lowercase();
        let mut start = 0;
        while let Some(pos) = line_lower[start..].find(&query_lower) {
            let match_start = start + pos;
            state.ui.search.matches.push(SearchMatch {
                line: *line_idx,
                start_col: match_start,
                end_col: match_start + query.len(),
            });
            start = match_start + 1;
        }
    }

    // Scroll to first match if any found
    if !state.ui.search.matches.is_empty() {
        scroll_to_current_match(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that UTF-8 string truncation handles multi-byte characters safely.
    /// This pattern is used in paste_to_terminal for logging preview.
    /// Previously used unsafe byte slicing: &content[..content.len().min(50)]
    /// which panics when byte 50 falls mid-character.
    #[test]
    fn test_safe_utf8_truncation() {
        // ASCII-only string - should work with both methods
        let ascii = "Hello, World!";
        let preview: String = ascii.chars().take(50).collect();
        assert_eq!(preview, "Hello, World!");

        // String with emoji at the boundary
        // Each emoji is 4 bytes. 12 emojis = 48 bytes, then "ab" = 50 bytes total
        // Byte slicing at 50 would work here, but let's test with 49
        let emoji_boundary = "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉ab";
        assert_eq!(emoji_boundary.len(), 50); // 12*4 + 2 = 50 bytes
        let preview: String = emoji_boundary.chars().take(50).collect();
        assert_eq!(preview, emoji_boundary); // Should get all 14 chars

        // String where byte 50 falls in middle of emoji
        // 12 emojis = 48 bytes, then one more emoji starts at byte 48
        // Byte 50 would be in the middle of the 13th emoji
        let mid_emoji = "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉"; // 13 emojis = 52 bytes
        assert_eq!(mid_emoji.len(), 52);
        // Byte slice &mid_emoji[..50] would PANIC here!
        // But chars().take(50) safely returns all 13 emojis
        let preview: String = mid_emoji.chars().take(50).collect();
        assert_eq!(preview, mid_emoji);
        assert_eq!(preview.chars().count(), 13);

        // Mixed ASCII and multi-byte
        let mixed = "Hello 🌍 World 🚀 Test 🎯 Done";
        let preview: String = mixed.chars().take(10).collect();
        assert_eq!(preview, "Hello 🌍 Wo");

        // Very short string
        let short = "Hi";
        let preview: String = short.chars().take(50).collect();
        assert_eq!(preview, "Hi");

        // Empty string
        let empty = "";
        let preview: String = empty.chars().take(50).collect();
        assert_eq!(preview, "");

        // Japanese text (3 bytes per char)
        let japanese = "こんにちは世界"; // 7 chars, 21 bytes
        let preview: String = japanese.chars().take(5).collect();
        assert_eq!(preview, "こんにちは");
    }

    // ── URL detection tests ────────────────────────────────────────

    #[test]
    fn detect_https_url() {
        let urls = detect_urls_in_line("visit https://example.com/path for info", 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com/path");
        assert_eq!(urls[0].start_col, 6);
        assert_eq!(urls[0].line, 0);
    }

    #[test]
    fn detect_http_url() {
        let urls = detect_urls_in_line("http://localhost:8080/api", 5);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "http://localhost:8080/api");
        assert_eq!(urls[0].line, 5);
    }

    #[test]
    fn detect_file_url() {
        let urls = detect_urls_in_line("file:///home/user/doc.txt", 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "file:///home/user/doc.txt");
    }

    #[test]
    fn detect_www_prefix() {
        let urls = detect_urls_in_line("go to www.example.com/page", 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "www.example.com/page");
    }

    #[test]
    fn detect_url_with_query_and_fragment() {
        let urls = detect_urls_in_line("https://example.com/path?q=hello&lang=en#section", 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com/path?q=hello&lang=en#section");
    }

    #[test]
    fn detect_url_trims_trailing_sentence_punctuation() {
        let urls = detect_urls_in_line("see https://example.com/path.", 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com/path");
        // end_col must shrink with the trimmed text so hit-testing still lines up.
        assert_eq!(urls[0].end_col, urls[0].start_col + "https://example.com/path".len());
    }

    #[test]
    fn detect_url_trims_multiple_trailing_punctuation() {
        // Wrapped in parens and followed by a comma: "(https://example.com),"
        let urls = detect_urls_in_line("(https://example.com),", 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
    }

    #[test]
    fn detect_url_keeps_balanced_trailing_paren() {
        let urls = detect_urls_in_line(
            "https://en.wikipedia.org/wiki/Rust_(programming_language)",
            0,
        );
        assert_eq!(urls.len(), 1);
        assert_eq!(
            urls[0].url,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        );
    }

    #[test]
    fn detect_url_drops_unbalanced_trailing_paren_keeps_inner() {
        // Inner balanced paren kept; the extra wrapping ')' dropped.
        let urls = detect_urls_in_line("(https://ex.com/a_(b)_c)", 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://ex.com/a_(b)_c");
    }

    #[test]
    fn detect_www_trims_trailing_period() {
        let urls = detect_urls_in_line("go to www.example.com/page.", 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "www.example.com/page");
    }

    #[test]
    fn detect_multiple_urls_in_line() {
        let urls = detect_urls_in_line("see https://a.com and https://b.com", 0);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].url, "https://a.com");
        assert_eq!(urls[1].url, "https://b.com");
    }

    #[test]
    fn detect_no_urls_in_plain_text() {
        let urls = detect_urls_in_line("just some plain text here", 0);
        assert_eq!(urls.len(), 0);
    }

    #[test]
    fn find_url_at_position_hit() {
        let urls = detect_urls_in_line("go to https://example.com now", 0);
        assert!(find_url_at_position(&urls, 10, 0).is_some());
    }

    #[test]
    fn find_url_at_position_miss() {
        let urls = detect_urls_in_line("go to https://example.com now", 0);
        // Position 0 is before the URL
        assert!(find_url_at_position(&urls, 0, 0).is_none());
        // Wrong line
        assert!(find_url_at_position(&urls, 10, 1).is_none());
    }

    #[test]
    fn find_url_index_at_position_returns_index() {
        let urls = detect_urls_in_line("https://a.com https://b.com", 0);
        assert_eq!(find_url_index_at_position(&urls, 0, 0), Some(0));
        assert_eq!(find_url_index_at_position(&urls, 15, 0), Some(1));
    }

    // ── File path detection tests ──────────────────────────────────

    #[test]
    fn detect_absolute_path() {
        let paths = detect_paths_in_line("see /etc/hosts for config", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "/etc/hosts");
        assert_eq!(paths[0].target_line, None);
        assert!(!paths[0].exists);
    }

    #[test]
    fn detect_home_path() {
        let paths = detect_paths_in_line("edit ~/.config/crt/config.toml", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "~/.config/crt/config.toml");
    }

    #[test]
    fn detect_relative_dot_path() {
        let paths = detect_paths_in_line("run ./scripts/build.sh now", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "./scripts/build.sh");
    }

    #[test]
    fn detect_relative_parent_path() {
        let paths = detect_paths_in_line("../sibling/file.rs", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "../sibling/file.rs");
    }

    #[test]
    fn detect_bare_multi_segment_path() {
        let paths = detect_paths_in_line("open src/window/mod.rs please", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "src/window/mod.rs");
    }

    #[test]
    fn detect_ignores_single_segment_word() {
        let paths = detect_paths_in_line("just README and Cargo.toml words", 0);
        assert_eq!(paths.len(), 0);
    }

    #[test]
    fn detect_ignores_urls() {
        let paths = detect_paths_in_line("go to https://example.com/path now", 0);
        assert_eq!(paths.len(), 0);
        let paths = detect_paths_in_line("file:///home/user/doc.txt", 0);
        assert_eq!(paths.len(), 0);
    }

    #[test]
    fn detect_path_with_line_suffix() {
        let paths = detect_paths_in_line("error at src/main.rs:42 here", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "src/main.rs");
        assert_eq!(paths[0].target_line, Some(42));
        assert_eq!(paths[0].target_col, None);
    }

    #[test]
    fn detect_path_with_line_and_col_suffix() {
        let paths = detect_paths_in_line("at src/main.rs:42:7", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "src/main.rs");
        assert_eq!(paths[0].target_line, Some(42));
        assert_eq!(paths[0].target_col, Some(7));
    }

    #[test]
    fn detect_path_colon_in_name_not_a_suffix() {
        // A trailing `:word` (non-numeric) is part of the path, not a suffix.
        let paths = detect_paths_in_line("foo/bar:baz", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "foo/bar:baz");
        assert_eq!(paths[0].target_line, None);
    }

    #[test]
    fn detect_path_trims_trailing_sentence_punctuation() {
        let paths = detect_paths_in_line("see /etc/hosts.", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "/etc/hosts");
    }

    #[test]
    fn detect_path_span_includes_suffix() {
        // The clickable span covers the whole `src/main.rs:42` token.
        let paths = detect_paths_in_line("src/main.rs:42", 0);
        assert_eq!(paths[0].start_col, 0);
        assert_eq!(paths[0].end_col, "src/main.rs:42".chars().count());
    }

    #[test]
    fn detect_multiple_paths_in_line() {
        let paths = detect_paths_in_line("diff a/src/x.rs b/src/y.rs", 0);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].path, "a/src/x.rs");
        assert_eq!(paths[1].path, "b/src/y.rs");
    }

    #[test]
    fn detect_double_quoted_path_with_spaces() {
        let paths = detect_paths_in_line(r#"open "/Users/me/My Docs/file.txt" now"#, 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "/Users/me/My Docs/file.txt");
    }

    #[test]
    fn detect_single_quoted_path_with_spaces() {
        let paths = detect_paths_in_line("edit '~/My Folder/a.rs' please", 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "~/My Folder/a.rs");
    }

    #[test]
    fn detect_backslash_escaped_spaces() {
        let paths = detect_paths_in_line(r"cat /Users/me/My\ Docs/file.txt", 0);
        assert_eq!(paths.len(), 1);
        // The escaped spaces are unescaped in the resolved path...
        assert_eq!(paths[0].path, "/Users/me/My Docs/file.txt");
        // ...but the underline span covers the on-screen token (with backslashes).
        let on_screen = r"/Users/me/My\ Docs/file.txt";
        assert_eq!(paths[0].end_col - paths[0].start_col, on_screen.chars().count());
    }

    #[test]
    fn detect_quoted_path_span_includes_quotes() {
        let paths = detect_paths_in_line(r#""a/b c.rs""#, 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].start_col, 0);
        // Span covers both quotes.
        assert_eq!(paths[0].end_col, r#""a/b c.rs""#.chars().count());
    }

    #[test]
    fn detect_quoted_path_with_line_suffix() {
        let paths = detect_paths_in_line(r#""my docs/main.rs:42""#, 0);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].path, "my docs/main.rs");
        assert_eq!(paths[0].target_line, Some(42));
    }

    #[test]
    fn detect_two_quoted_paths_in_line() {
        let paths = detect_paths_in_line(r#"diff "a/x y.rs" "b/z w.rs""#, 0);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].path, "a/x y.rs");
        assert_eq!(paths[1].path, "b/z w.rs");
    }

    #[test]
    fn validate_spaced_path_is_clickable() {
        // End-to-end: a real file whose name contains a space resolves + validates.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("My Docs")).unwrap();
        std::fs::write(dir.path().join("My Docs/file.txt"), b"x").unwrap();

        let mut p = detect_paths_in_line(r#"see "My Docs/file.txt""#, 0)
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(p.path, "My Docs/file.txt");

        let mut validator = PathValidator::default();
        validator.begin_pass(Some(dir.path().to_path_buf()), None);
        validator.validate(&mut p);
        assert!(p.exists);
    }

    #[test]
    fn parse_path_suffix_variants() {
        assert_eq!(parse_path_suffix("src/a.rs"), ("src/a.rs", None, None));
        assert_eq!(parse_path_suffix("src/a.rs:5"), ("src/a.rs", Some(5), None));
        assert_eq!(parse_path_suffix("src/a.rs:5:9"), ("src/a.rs", Some(5), Some(9)));
        assert_eq!(parse_path_suffix("a:b"), ("a:b", None, None));
    }

    #[test]
    fn find_path_at_position_hit_and_miss() {
        let paths = detect_paths_in_line("open src/main.rs now", 0);
        // "open " is 5 chars, so the path starts at col 5.
        assert!(find_path_at_position(&paths, 5, 0).is_some());
        assert!(find_path_at_position(&paths, 0, 0).is_none());
        assert!(find_path_at_position(&paths, 5, 1).is_none());
    }

    #[test]
    fn find_path_index_at_position_returns_index() {
        let paths = detect_paths_in_line("a/x.rs b/y.rs", 0);
        assert_eq!(find_path_index_at_position(&paths, 0, 0), Some(0));
        assert_eq!(find_path_index_at_position(&paths, 7, 0), Some(1));
    }

    // ── Path resolution & validation tests ─────────────────────────

    #[test]
    fn resolve_path_absolute() {
        assert_eq!(
            resolve_path("/etc/hosts", None, None),
            Some(PathBuf::from("/etc/hosts"))
        );
    }

    #[test]
    fn resolve_path_home() {
        let home = PathBuf::from("/home/me");
        assert_eq!(resolve_path("~", None, Some(&home)), Some(home.clone()));
        assert_eq!(
            resolve_path("~/.config/x", None, Some(&home)),
            Some(PathBuf::from("/home/me/.config/x"))
        );
        // No home available → cannot resolve.
        assert_eq!(resolve_path("~/x", None, None), None);
        // ~user form unsupported.
        assert_eq!(resolve_path("~other/x", None, Some(&home)), None);
    }

    #[test]
    fn resolve_path_relative_needs_cwd() {
        let cwd = PathBuf::from("/work/project");
        assert_eq!(
            resolve_path("src/main.rs", Some(&cwd), None),
            Some(PathBuf::from("/work/project/src/main.rs"))
        );
        // No cwd → cannot resolve a relative path.
        assert_eq!(resolve_path("src/main.rs", None, None), None);
    }

    #[test]
    fn validate_marks_existing_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("real.txt"), b"hi").unwrap();

        let mut existing = DetectedPath {
            path: "real.txt".to_string(),
            target_line: None,
            target_col: None,
            exists: false,
            start_col: 0,
            end_col: 8,
            line: 0,
            end_line: 0,
        };
        let mut missing = DetectedPath {
            path: "nope.txt".to_string(),
            ..existing.clone()
        };

        let mut validator = PathValidator::default();
        validator.begin_pass(Some(dir.path().to_path_buf()), None);
        validator.validate(&mut existing);
        validator.validate(&mut missing);

        assert!(existing.exists);
        assert!(!missing.exists);
    }

    #[test]
    fn validate_expands_home() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cfg"), b"x").unwrap();

        let mut p = DetectedPath {
            path: "~/cfg".to_string(),
            target_line: None,
            target_col: None,
            exists: false,
            start_col: 0,
            end_col: 5,
            line: 0,
            end_line: 0,
        };

        let mut validator = PathValidator::default();
        validator.begin_pass(None, Some(dir.path().to_path_buf()));
        validator.validate(&mut p);
        assert!(p.exists);
    }

    #[test]
    fn validate_caches_repeated_checks() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"x").unwrap();

        let mk = || DetectedPath {
            path: "a.txt".to_string(),
            target_line: None,
            target_col: None,
            exists: false,
            start_col: 0,
            end_col: 5,
            line: 0,
            end_line: 0,
        };

        let mut validator = PathValidator::default();
        validator.begin_pass(Some(dir.path().to_path_buf()), None);
        let mut p1 = mk();
        let mut p2 = mk();
        validator.validate(&mut p1);
        validator.validate(&mut p2);

        // Second check hit the cache → only one filesystem stat performed.
        assert_eq!(validator.stats_this_pass, 1);
        assert!(p1.exists && p2.exists);
    }

    #[test]
    fn validate_cache_invalidates_on_cwd_change() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        // "file" exists only under dir_a.
        std::fs::write(dir_a.path().join("file"), b"x").unwrap();

        let mk = || DetectedPath {
            path: "file".to_string(),
            target_line: None,
            target_col: None,
            exists: false,
            start_col: 0,
            end_col: 4,
            line: 0,
            end_line: 0,
        };

        let mut validator = PathValidator::default();

        validator.begin_pass(Some(dir_a.path().to_path_buf()), None);
        let mut pa = mk();
        validator.validate(&mut pa);
        assert!(pa.exists);

        // Switch cwd → cache cleared, "file" resolves under dir_b where it is absent.
        validator.begin_pass(Some(dir_b.path().to_path_buf()), None);
        let mut pb = mk();
        validator.validate(&mut pb);
        assert!(!pb.exists);
        assert_eq!(validator.stats_this_pass, 1);
    }

    /// Build a single-line `DetectedPath` for a token (test helper).
    fn dp(path: &str) -> DetectedPath {
        DetectedPath {
            path: path.to_string(),
            target_line: None,
            target_col: None,
            exists: false,
            start_col: 0,
            end_col: path.chars().count(),
            line: 0,
            end_line: 0,
        }
    }

    #[test]
    fn validate_directory_is_clickable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        let mut p = dp("subdir");
        let mut validator = PathValidator::default();
        validator.begin_pass(Some(dir.path().to_path_buf()), None);
        validator.validate(&mut p);
        // Directories exist → clickable (opened via the OS default handler).
        assert!(p.exists);
    }

    #[test]
    fn validate_line_suffix_on_missing_file_not_clickable() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = detect_paths_in_line("see src/nope.rs:42 here", 0)
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(p.path, "src/nope.rs");
        assert_eq!(p.target_line, Some(42));
        let mut validator = PathValidator::default();
        validator.begin_pass(Some(dir.path().to_path_buf()), None);
        validator.validate(&mut p);
        assert!(!p.exists);
    }

    #[cfg(unix)]
    #[test]
    fn validate_follows_valid_symlink() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("target.txt"), b"x").unwrap();
        std::os::unix::fs::symlink(
            dir.path().join("target.txt"),
            dir.path().join("link.txt"),
        )
        .unwrap();
        let mut p = dp("link.txt");
        let mut validator = PathValidator::default();
        validator.begin_pass(Some(dir.path().to_path_buf()), None);
        validator.validate(&mut p);
        assert!(p.exists);
    }

    #[test]
    fn validate_caches_full_screen_of_duplicates() {
        // A screenful of the same path → exactly one filesystem check (NFR-001).
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), b"x").unwrap();
        let mut paths: Vec<DetectedPath> = (0..200).map(|_| dp("a.rs")).collect();
        let mut validator = PathValidator::default();
        validator.begin_pass(Some(dir.path().to_path_buf()), None);
        validator.validate_all(&mut paths);
        assert_eq!(validator.stats_this_pass, 1);
        assert!(paths.iter().all(|p| p.exists));
    }

    #[test]
    fn validate_respects_per_pass_budget() {
        // More distinct (missing) tokens than the budget → checks are capped and
        // the overflow is treated as non-existent rather than stalling.
        let dir = tempfile::tempdir().unwrap();
        let mut paths: Vec<DetectedPath> = (0..MAX_STAT_PER_PASS + 10)
            .map(|i| dp(&format!("missing/{i}.rs")))
            .collect();
        let mut validator = PathValidator::default();
        validator.begin_pass(Some(dir.path().to_path_buf()), None);
        validator.validate_all(&mut paths);
        assert_eq!(validator.stats_this_pass, MAX_STAT_PER_PASS);
        assert!(paths.iter().all(|p| !p.exists));
    }

    // ── open_file command builder tests ────────────────────────────

    #[test]
    fn build_open_command_file_only() {
        let (prog, args) = build_open_command("subl {file}", "/a/b.rs", None, None).unwrap();
        assert_eq!(prog, "subl");
        assert_eq!(args, vec!["/a/b.rs".to_string()]);
    }

    #[test]
    fn build_open_command_with_line() {
        let (prog, args) =
            build_open_command("code -g {file}:{line}", "/a/b.rs", Some(42), None).unwrap();
        assert_eq!(prog, "code");
        assert_eq!(args, vec!["-g".to_string(), "/a/b.rs:42".to_string()]);
    }

    #[test]
    fn build_open_command_with_line_and_col() {
        let (_prog, args) =
            build_open_command("code -g {file}:{line}:{col}", "/a/b.rs", Some(42), Some(7)).unwrap();
        assert_eq!(args, vec!["-g".to_string(), "/a/b.rs:42:7".to_string()]);
    }

    #[test]
    fn build_open_command_defaults_missing_line_col() {
        let (_prog, args) =
            build_open_command("e {file}:{line}:{col}", "/a/b.rs", None, None).unwrap();
        assert_eq!(args, vec!["/a/b.rs:1:1".to_string()]);
    }

    #[test]
    fn build_open_command_path_with_spaces_stays_one_arg() {
        let (_prog, args) = build_open_command("code -g {file}", "/a b/c d.rs", None, None).unwrap();
        assert_eq!(args, vec!["-g".to_string(), "/a b/c d.rs".to_string()]);
    }

    #[test]
    fn build_open_command_empty_template_is_none() {
        assert!(build_open_command("   ", "/a", None, None).is_none());
    }

    // ── Mouse report protocol tests ────────────────────────────────

    #[test]
    fn mouse_report_sgr_press() {
        let seq = mouse_report(0, 5, 10, true, true);
        let expected = "\x1b[<0;6;11M"; // SGR: 1-indexed, M for press
        assert_eq!(String::from_utf8(seq).unwrap(), expected);
    }

    #[test]
    fn mouse_report_sgr_release() {
        let seq = mouse_report(0, 5, 10, false, true);
        let expected = "\x1b[<0;6;11m"; // SGR: lowercase m for release
        assert_eq!(String::from_utf8(seq).unwrap(), expected);
    }

    #[test]
    fn mouse_report_legacy_press() {
        let seq = mouse_report(0, 0, 0, true, false);
        // Legacy: ESC [ M <btn+32> <col+33> <row+33>
        assert_eq!(seq, vec![0x1b, b'[', b'M', 32, 33, 33]);
    }

    #[test]
    fn mouse_report_legacy_release() {
        let seq = mouse_report(MOUSE_BUTTON_RELEASE, 0, 0, false, false);
        assert_eq!(seq, vec![0x1b, b'[', b'M', 35, 33, 33]); // 3+32=35
    }

    #[test]
    fn mouse_report_scroll_buttons() {
        let up = mouse_report(MOUSE_BUTTON_SCROLL_UP, 10, 5, true, true);
        let down = mouse_report(MOUSE_BUTTON_SCROLL_DOWN, 10, 5, true, true);
        assert_ne!(up, down);
        let up_str = String::from_utf8(up).unwrap();
        assert!(up_str.contains("<64;")); // scroll up = 64
        let down_str = String::from_utf8(down).unwrap();
        assert!(down_str.contains("<65;")); // scroll down = 65
    }

    #[test]
    fn mouse_report_legacy_clamps_coordinates() {
        // Legacy mode clamps to 255
        let seq = mouse_report(0, 300, 300, true, false);
        // col+33 and row+33 should be clamped to 255
        assert_eq!(seq[4], 255);
        assert_eq!(seq[5], 255);
    }

    // ── URL merge tests ────────────────────────────────────────────

    #[test]
    fn merge_wrapped_urls_single_line() {
        let mut urls = detect_urls_in_line("https://short.com", 0);
        let mut lines = std::collections::BTreeMap::new();
        lines.insert(0, "https://short.com".to_string());
        merge_wrapped_urls(&mut urls, &lines, 80);
        // No merge should happen — URL doesn't end at column boundary
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].end_line, 0);
    }

    #[test]
    fn merge_wrapped_urls_across_lines() {
        // URL that ends at column boundary (col 20)
        let line0 = "https://example.com/";
        let line1 = "very/long/path/here";
        let mut urls = detect_urls_in_line(line0, 0);
        let mut lines = std::collections::BTreeMap::new();
        lines.insert(0, line0.to_string());
        lines.insert(1, line1.to_string());
        merge_wrapped_urls(&mut urls, &lines, 20);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].end_line, 1);
        assert!(urls[0].url.contains("very/long/path/here"));
    }

    #[test]
    fn merge_wrapped_urls_stops_at_new_protocol() {
        let line0 = "https://example.com/";
        let line1 = "https://other.com";
        let mut urls = detect_urls_in_line(line0, 0);
        let mut lines = std::collections::BTreeMap::new();
        lines.insert(0, line0.to_string());
        lines.insert(1, line1.to_string());
        merge_wrapped_urls(&mut urls, &lines, 20);
        // Should NOT merge — next line starts with https://
        assert_eq!(urls[0].end_line, 0);
    }
}
