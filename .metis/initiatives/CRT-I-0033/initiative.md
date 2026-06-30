---
id: clickable-file-path-opener
level: initiative
title: "Clickable File Path Opener"
short_code: "CRT-I-0033"
created_at: 2026-06-30T15:44:21.254880+00:00
updated_at: 2026-06-30T15:44:21.254880+00:00
parent: 
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
initiative_id: clickable-file-path-opener
---

# Clickable File Path Opener Initiative

## Context **[REQUIRED]**

CRT already supports Cmd+click (Super on macOS, Ctrl on Linux) to open URLs detected in
the terminal viewport. The feature lives in `src/input/mod.rs` (detection: `DetectedUrl`,
`detect_urls_in_line`, `merge_wrapped_urls`, `find_url_at_position`,
`find_url_index_at_position`, `is_position_in_url`, `trim_url_trailing_punctuation`,
`open_url`), is populated per-frame in `src/window/mod.rs` (~L302–313), hover-tracked in
`src/input/mouse.rs` (`update_hover`, `hovered_url_index`), opened on Cmd+click in
`src/input/mouse.rs` (~L235–246, `MouseClickTarget::OpenUrl`), and rendered as a hover
underline in `src/window/render.rs` (~L219–239, `RenderContext.detected_urls` /
`hovered_url_index`).

This initiative adds a **parallel capability for local file paths**: detect path-like
text in the viewport, make valid (on-disk) paths clickable, and open them on Cmd+click.

Critical enabler already present: the shell's working directory is available via
`ShellTerminal::working_directory()` (`crates/crt-core/src/lib.rs` →
`crates/crt-core/src/pty.rs::get_process_cwd`, resolved from the foreground process via
`lsof` on macOS and `/proc/<pid>/cwd` on Linux), exposed to the window layer as
`active_shell_cwd()` (`src/window/mod.rs` ~L529). This makes CWD-relative resolution and
on-disk existence validation feasible.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- Detect file paths in the viewport: absolute (`/foo/bar`), home (`~/foo`), and relative
  (`./foo`, `../foo`, bare `src/main.rs`) forms.
- Resolve relative paths against the shell CWD and make a path clickable **only if it
  exists on disk** (validation is what keeps relative-path detection usable).
- Cmd+click (same modifier as URLs) opens the file via the OS default
  (`open::that`), with an **optional configurable editor command** that takes precedence
  when set (e.g. `code -g {file}:{line}`).
- Support `:line[:col]` targeting: a trailing suffix (`src/main.rs:42`) is parsed and
  passed to the configured editor command; ignored for OS-default opens.
- Hover affordance (underline) consistent with the existing URL hover.

**Non-Goals:**
- No remote/SSH path resolution; CWD comes from the local foreground process only.
- No re-handling of `file://` URLs — those remain the URL opener's responsibility.
- No directory-aware navigation UI (e.g. opening a file picker); a click opens the target.
- No reconstruction of the CWD that was active when *scrollback* output was produced; we
  resolve against the *current* foreground CWD (documented limitation).

## Requirements

### Functional Requirements
- REQ-001: Detect absolute, home (`~`), and relative path tokens in each viewport line.
- REQ-002: Resolve `~` against `$HOME` and relatives against `active_shell_cwd()`.
- REQ-003: A token is clickable only if the resolved path exists on disk.
- REQ-004: Parse an optional trailing `:line` or `:line:col` suffix; the suffix is not
  part of the path used for the existence check.
- REQ-005: Cmd/Ctrl+click on a valid path opens it. If a URL also matches at that
  position, the URL takes precedence (file:// stays a URL).
- REQ-006: Default open is `open::that`. If `open_file_command` is configured, run it,
  substituting `{file}`, `{line}`, `{col}` placeholders.
- REQ-007: Hovering a valid path underlines it like a URL.

### Non-Functional Requirements
- NFR-001: Per-frame path detection + existence checks must not regress frame time.
  Existence checks must be cached/throttled (see Detailed Design) — no unbounded `stat`
  storm every frame.
- NFR-002: Cross-platform (macOS + Linux) parity with the URL feature's CI constraints.

## Use Cases

### Use Case 1: Open a source file from compiler/grep output
- **Actor**: Developer
- **Scenario**: Runs `cargo build`/`rg`, output shows `src/window/mod.rs:529`. Holds Cmd
  and clicks the path.
- **Expected Outcome**: File opens (in `$EDITOR`/configured editor at line 529 if a command
  is configured, otherwise OS default app).

### Use Case 2: Open an absolute/home path
- **Actor**: User
- **Scenario**: Output contains `~/.config/crt/config.toml` or `/etc/hosts`; Cmd+click.
- **Expected Outcome**: File opens via the configured/default opener.

## Architecture

### Overview
Mirror the URL pipeline with a parallel "detected path" concept, reusing the existing
hover/render machinery. Two viable structural approaches (decision in Detailed Design):

- **(A) Parallel detector** — add `DetectedPath` + `detected_paths` + `hovered_path_index`
  alongside the URL equivalents. Lowest risk, some duplication.
- **(B) Unified link concept** — generalize `DetectedUrl` into a `DetectedLink { kind:
  Url | File, ... }` so hover/render handle one collection. Less duplication, larger
  blast radius on existing URL code/tests.

Recommended: start with (A) for isolation and test parity, factor toward (B) only if the
duplication proves costly.

### Key components touched
- `src/input/mod.rs` — new path detector, suffix parser, resolver/validator, `open_file`.
- `src/window/mod.rs` — per-frame population (needs `active_shell_cwd()` + `$HOME`).
- `src/input/mouse.rs` — hover update + Cmd+click target/dispatch with URL precedence.
- `src/window/render.rs` — hover underline for paths.
- `src/config.rs` — new `open_file_command` option.

## Detailed Design **[REQUIRED]**

**Detection.** A path-candidate regex/tokenizer over each viewport line recognizes
tokens beginning with `/`, `~/`, `./`, `../`, or a bare `segment/segment...` containing a
`/` (single-segment bare words like `README` are excluded to bound false positives). As
with URLs, candidates are trimmed of trailing prose punctuation
(reuse/share `trim_url_trailing_punctuation` logic). Multi-line wrapped paths can reuse
the `merge_wrapped_urls` approach if needed (lower priority — paths wrap less often).

**Suffix parsing.** Before validation, split a trailing `:(\d+)(:(\d+))?`. The numeric
suffix is captured as `line`/`col` and stripped from the path string used for stat.
(Guard against `:` in the path itself being misread — only a *trailing* numeric suffix
counts.)

**Resolution + validation.** `~` → `$HOME`; relative → `active_shell_cwd()`; absolute as-is.
`std::path::Path::exists()` (or `try_exists`) gates clickability. To satisfy NFR-001, cache
results: detection already runs only when the rendered line set changes; additionally cache
`(cwd, token) -> bool` for the current frame set and avoid re-stat'ing unchanged lines.
Bound the number of candidates stat'd per frame.

**Opening.** `open_file(path, line, col, cfg)`: if `cfg.open_file_command` is set, spawn it
with `{file}`/`{line}`/`{col}` substituted (sensible defaults when no suffix, e.g. line 1);
else `open::that(path)` (ignoring line/col). Mirror `open_url`'s error logging.

**Precedence.** In the Cmd+click handler, check URL hit first (existing
`find_url_at_position`), then path hit. This keeps `file://` and http(s) behavior intact.

### Open design questions (resolve during design phase)
- Approach (A) vs (B) above.
- Exact bare-relative grammar (how aggressive) and whether to require ≥1 `/`.
- Caching granularity to meet NFR-001.
- Default `open_file_command` placeholder semantics + docs.

## Testing Strategy
- **Unit** (mirror existing URL tests in `src/input/mod.rs`): detection of each path form;
  suffix parsing (`:42`, `:42:7`, no suffix, colon-in-name); trailing punctuation trim;
  position hit/miss; resolution of `~`/relative/absolute; existence gating with temp dirs.
- **Unit**: `open_file` command-string substitution (without actually spawning), and the
  URL-vs-path precedence decision in the pure click-target function.
- **Integration**: per-frame population given a mock CWD; no stat storm (assert caching).
- Respect existing CI constraints (visual/golden tests are macOS-only per repo history).

## Alternatives Considered **[REQUIRED]**

- **Absolute & home paths only (no CWD/existence):** simpler, no stat cost, but misses the
  most common case (relative paths in build/grep output). Rejected per design decision.
- **Detect any path-like token without existence validation:** maximal reach but heavy
  false positives (every `and/or`, every `a/b` fragment becomes clickable). Rejected.
- **Editor-command-only (no OS default):** maximum control but requires config to work at
  all; worse out-of-box UX. Rejected in favor of OS-default-with-override.
- **Reusing OSC 8 hyperlinks only:** doesn't help with plain compiler/grep output, which is
  the primary use case.

## Implementation Plan **[REQUIRED]**

Phased; to be decomposed into tasks after design sign-off (human-in-the-loop):
1. **Detection core** — `DetectedPath`, path tokenizer, suffix parser, trailing-trim; unit
   tests. (No I/O yet.)
2. **Resolution + validation** — `~`/CWD resolution, existence gating, caching; unit tests
   with temp dirs.
3. **Config + open action** — `open_file_command` in `config.rs`, `open_file` with
   placeholder substitution and OS-default fallback; unit tests.
4. **Wiring** — per-frame population in `window/mod.rs`, hover in `mouse.rs`, Cmd+click
   dispatch with URL precedence.
5. **Render** — hover underline for paths in `render.rs`.
6. **Polish/tests** — edge cases, NFR-001 verification, docs for the config option.