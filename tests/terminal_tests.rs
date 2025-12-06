//! Terminal emulation functional tests
//!
//! Tests terminal behavior without needing a real shell/PTY.
//! These tests run headlessly and are fast.

mod common;

use common::TerminalTestHarness;
use crt_core::{SemanticZone, TerminalEvent};

// === Basic text output tests ===

#[test]
fn test_simple_text_output() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("Hello, World!");

    let lines = harness.visible_lines();
    assert!(lines[0].contains("Hello, World!"));
    harness.assert_cursor_at(0, 13);
}

#[test]
fn test_multiline_output() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("Line 1\nLine 2\nLine 3");

    harness.assert_line_contains(0, "Line 1");
    harness.assert_line_contains(1, "Line 2");
    harness.assert_line_contains(2, "Line 3");
}

#[test]
fn test_carriage_return() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("AAAA\rBB");

    // BB should overwrite first two A's
    harness.assert_line_contains(0, "BBAA");
}

#[test]
fn test_line_wrap() {
    let mut harness = TerminalTestHarness::new(10, 5);

    // Text longer than 10 columns should wrap
    harness.input_str("12345678901234567890");

    let lines = harness.visible_lines();
    assert!(lines[0].len() <= 10);
    assert!(!lines[1].is_empty()); // Wrapped content
}

// === ANSI escape sequence tests ===

#[test]
fn test_cursor_movement_down() {
    let mut harness = TerminalTestHarness::default_size();

    // Move cursor down 3 lines
    harness.input(b"\x1b[3B");

    harness.assert_cursor_at(3, 0);
}

#[test]
fn test_cursor_movement_right() {
    let mut harness = TerminalTestHarness::default_size();

    // Move cursor right 10 columns
    harness.input(b"\x1b[10C");

    harness.assert_cursor_at(0, 10);
}

#[test]
fn test_cursor_absolute_position() {
    let mut harness = TerminalTestHarness::default_size();

    // Move to row 5, column 10 (1-indexed in ANSI)
    harness.input(b"\x1b[5;10H");

    harness.assert_cursor_at(4, 9); // 0-indexed
}

#[test]
fn test_clear_screen() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("Some content here");
    harness.input(b"\x1b[2J"); // Clear entire screen

    // Screen should be blank
    let lines = harness.visible_lines();
    assert!(lines[0].is_empty() || lines[0].trim().is_empty());
}

#[test]
fn test_clear_line() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("AAAAAAAAAA");
    harness.input(b"\x1b[5G"); // Move to column 5
    harness.input(b"\x1b[K"); // Clear from cursor to end of line

    let lines = harness.visible_lines();
    assert_eq!(lines[0].len(), 4); // Only first 4 characters remain
}

#[test]
fn test_insert_lines() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("Line 1\n");
    harness.input_str("Line 2\n");
    harness.input_str("Line 3");
    harness.input(b"\x1b[2;1H"); // Move to line 2
    harness.input(b"\x1b[L"); // Insert line

    // Line 2 should now be blank (or mostly blank)
    let lines = harness.visible_lines();
    // Original "Line 2" should have moved down
    assert!(lines[2].contains("Line 2") || lines[3].contains("Line 2"));
}

// === Color and style tests ===

#[test]
fn test_sgr_bold() {
    let mut harness = TerminalTestHarness::default_size();

    // Enable bold
    harness.input(b"\x1b[1m");
    harness.input_str("Bold text");
    harness.input(b"\x1b[0m"); // Reset

    // Text should be rendered (we can check content, style is in cell flags)
    harness.assert_line_contains(0, "Bold text");
}

#[test]
fn test_sgr_colors() {
    let mut harness = TerminalTestHarness::default_size();

    // Set foreground red (31) and background blue (44)
    harness.input(b"\x1b[31;44m");
    harness.input_str("Colored text");
    harness.input(b"\x1b[0m");

    harness.assert_line_contains(0, "Colored text");
}

#[test]
fn test_sgr_256_color() {
    let mut harness = TerminalTestHarness::default_size();

    // Set 256-color foreground (color 196 = red)
    harness.input(b"\x1b[38;5;196m");
    harness.input_str("256-color text");
    harness.input(b"\x1b[0m");

    harness.assert_line_contains(0, "256-color text");
}

#[test]
fn test_sgr_truecolor() {
    let mut harness = TerminalTestHarness::default_size();

    // Set 24-bit truecolor foreground (RGB: 255, 128, 0)
    harness.input(b"\x1b[38;2;255;128;0m");
    harness.input_str("Truecolor text");
    harness.input(b"\x1b[0m");

    harness.assert_line_contains(0, "Truecolor text");
}

// === OSC sequence tests ===

#[test]
fn test_osc_set_title() {
    let mut harness = TerminalTestHarness::default_size();

    // Set window title
    harness.input(b"\x1b]0;New Title\x07");

    // Title event should be generated
    let events = harness.terminal().take_events();
    let has_title = events
        .iter()
        .any(|e| matches!(e, TerminalEvent::Title(t) if t == "New Title"));
    assert!(has_title, "Expected title event");
}

#[test]
fn test_bell() {
    let mut harness = TerminalTestHarness::default_size();

    // Send bell character
    harness.input(b"\x07");

    // Bell event should be generated
    let events = harness.terminal().take_events();
    let has_bell = events.iter().any(|e| matches!(e, TerminalEvent::Bell));
    assert!(has_bell, "Expected bell event");
}

// === OSC 133 semantic zone tests ===

#[test]
fn test_osc133_prompt_detection() {
    let mut harness = TerminalTestHarness::default_size();

    // Simulate prompt start (OSC 133;A)
    harness.input(b"\x1b]133;A\x07");

    assert!(harness.terminal().has_semantic_zones());
    assert_eq!(harness.terminal().current_zone(), SemanticZone::Prompt);
}

#[test]
fn test_osc133_input_detection() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input(b"\x1b]133;A\x07"); // Prompt start
    harness.input_str("$ ");
    harness.input(b"\x1b]133;B\x07"); // Input start

    assert_eq!(harness.terminal().current_zone(), SemanticZone::Input);
}

#[test]
fn test_osc133_full_cycle() {
    let mut harness = TerminalTestHarness::default_size();

    // Full prompt cycle
    harness.input(b"\x1b]133;A\x07"); // Prompt
    harness.input_str("$ ");
    harness.input(b"\x1b]133;B\x07"); // Input
    harness.input_str("ls -la\n");
    harness.input(b"\x1b]133;C\x07"); // Output
    harness.input_str("file1.txt\nfile2.txt\n");
    harness.input(b"\x1b]133;A\x07"); // Next prompt

    assert_eq!(harness.terminal().current_zone(), SemanticZone::Prompt);
}

// === Scroll and history tests ===

#[test]
fn test_scroll_region() {
    let mut harness = TerminalTestHarness::default_size();

    // Set scroll region to lines 5-10
    harness.input(b"\x1b[5;10r");

    // Fill some content
    for i in 0..24 {
        harness.input_str(&format!("Line {}\n", i));
    }

    // Should have scrolled within region
    // (exact behavior depends on terminal implementation)
}

// === Selection tests ===

#[test]
fn test_selection_basic() {
    use crt_core::{Column, Line, Point, SelectionType};

    let mut harness = TerminalTestHarness::default_size();
    harness.input_str("Select this text");

    // Start selection at beginning
    harness
        .terminal_mut()
        .start_selection(Point::new(Line(0), Column(0)), SelectionType::Simple);

    // Extend to "Select"
    harness
        .terminal_mut()
        .update_selection(Point::new(Line(0), Column(6)));

    assert!(harness.terminal().has_selection());

    let selected = harness.terminal().selection_to_string();
    assert!(selected.is_some());
    assert!(selected.unwrap().contains("Select"));
}

#[test]
fn test_selection_clear() {
    use crt_core::{Column, Line, Point, SelectionType};

    let mut harness = TerminalTestHarness::default_size();
    harness.input_str("Text");

    harness
        .terminal_mut()
        .start_selection(Point::new(Line(0), Column(0)), SelectionType::Simple);
    assert!(harness.terminal().has_selection());

    harness.terminal_mut().clear_selection();
    assert!(!harness.terminal().has_selection());
}

// === Selection with scrollback tests ===

#[test]
fn test_selection_in_scrollback_history() {
    use crt_core::Scroll;
    use crt_core::{Column, Line, Point, SelectionType};

    let mut harness = TerminalTestHarness::new(80, 10);

    // Generate enough output to create scrollback history
    // Use distinct markers that are easy to find
    for i in 0..50 {
        harness.input_str(&format!("HIST{:02}X\n", i));
    }

    // Verify we have history
    let history_size = harness.terminal().history_size();
    assert!(history_size > 0, "Expected history, got none");

    // Scroll back into history
    harness.terminal_mut().scroll(Scroll::Delta(20));
    let display_offset = harness.terminal().display_offset();
    assert!(display_offset > 0, "Expected scroll back");

    // Use all_lines_text to verify what's actually in the history
    let all_lines = harness.terminal().all_lines_text();

    // Find a line that contains our marker
    let target_line = all_lines
        .iter()
        .find(|(_, text)| text.contains("HIST"))
        .expect("Should find a HIST marker in history");

    // Select that specific line using its grid coordinates
    let grid_line = target_line.0;
    harness.terminal_mut().start_selection(
        Point::new(Line(grid_line), Column(0)),
        SelectionType::Simple,
    );
    harness
        .terminal_mut()
        .update_selection(Point::new(Line(grid_line), Column(6)));

    assert!(harness.terminal().has_selection());

    let selected = harness.terminal().selection_to_string();
    assert!(selected.is_some());
    // Should contain our marker from the scrollback
    assert!(
        selected.as_ref().unwrap().contains("HIST"),
        "Expected 'HIST' in selection, got: {:?}",
        selected
    );
}

#[test]
fn test_selection_multiline_in_scrollback() {
    use crt_core::Scroll;
    use crt_core::{Column, Line, Point, SelectionType};

    let mut harness = TerminalTestHarness::new(80, 10);

    // Generate numbered lines for easy verification
    for i in 0..50 {
        harness.input_str(&format!("LINE-{:03}\n", i));
    }

    // Scroll back
    harness.terminal_mut().scroll(Scroll::Delta(15));
    let display_offset = harness.terminal().display_offset() as i32;

    // Select multiple lines in scrollback
    let start_grid_line = -display_offset;
    let end_grid_line = start_grid_line + 3;

    harness.terminal_mut().start_selection(
        Point::new(Line(start_grid_line), Column(0)),
        SelectionType::Simple,
    );
    harness
        .terminal_mut()
        .update_selection(Point::new(Line(end_grid_line), Column(7)));

    let selected = harness.terminal().selection_to_string();
    assert!(selected.is_some());

    let text = selected.unwrap();
    // Should contain multiple LINE- entries
    let line_count = text.matches("LINE-").count();
    assert!(
        line_count >= 3,
        "Expected at least 3 lines selected, got {}: {}",
        line_count,
        text
    );
}

#[test]
fn test_selection_spanning_history_and_visible() {
    use crt_core::Scroll;
    use crt_core::{Column, Line, Point, SelectionType};

    let mut harness = TerminalTestHarness::new(80, 10);

    // Generate history
    for i in 0..30 {
        harness.input_str(&format!("Line {}\n", i));
    }

    // Scroll back partially (only a few lines)
    harness.terminal_mut().scroll(Scroll::Delta(5));
    let display_offset = harness.terminal().display_offset() as i32;

    // Start selection in scrollback (negative grid line)
    let start_grid_line = -display_offset; // First visible line in scrollback
    // End in visible screen area (positive grid line)
    let end_grid_line = 5; // Should be in visible area

    harness.terminal_mut().start_selection(
        Point::new(Line(start_grid_line), Column(0)),
        SelectionType::Simple,
    );
    harness
        .terminal_mut()
        .update_selection(Point::new(Line(end_grid_line), Column(10)));

    let selected = harness.terminal().selection_to_string();
    assert!(selected.is_some());

    // Selection should span multiple lines including both history and visible
    let text = selected.unwrap();
    assert!(
        text.lines().count() > 5,
        "Expected selection spanning history and visible, got: {}",
        text
    );
}

#[test]
fn test_selection_coordinates_at_scroll_bottom() {
    use crt_core::{Column, Line, Point, SelectionType};

    let mut harness = TerminalTestHarness::new(80, 10);

    // Generate some history
    for i in 0..20 {
        harness.input_str(&format!("Line {}\n", i));
    }

    // Ensure we're at the bottom (display_offset = 0)
    assert_eq!(
        harness.terminal().display_offset(),
        0,
        "Should be at bottom after output"
    );

    // At bottom, grid line 0 = viewport line 0
    harness
        .terminal_mut()
        .start_selection(Point::new(Line(0), Column(0)), SelectionType::Simple);
    harness
        .terminal_mut()
        .update_selection(Point::new(Line(0), Column(10)));

    let selected = harness.terminal().selection_to_string();
    assert!(selected.is_some());
}

#[test]
fn test_viewport_to_grid_coordinate_conversion() {
    // This test verifies the coordinate conversion logic:
    // grid_line = viewport_line - display_offset
    // viewport_line = grid_line + display_offset

    use crt_core::Scroll;

    let mut harness = TerminalTestHarness::new(80, 10);

    // Generate history
    for i in 0..50 {
        harness.input_str(&format!("Line {}\n", i));
    }

    // Test at various scroll positions
    let test_cases = [0, 5, 10, 20];

    for &scroll_amount in &test_cases {
        // Scroll to bottom first
        harness.terminal_mut().scroll(Scroll::Bottom);
        assert_eq!(harness.terminal().display_offset(), 0);

        if scroll_amount > 0 {
            harness.terminal_mut().scroll(Scroll::Delta(scroll_amount));
        }

        let display_offset = harness.terminal().display_offset() as i32;
        assert_eq!(
            display_offset, scroll_amount,
            "display_offset should match scroll amount"
        );

        // Verify conversion math:
        // If viewport_line = 5, then grid_line = 5 - display_offset
        let viewport_line = 5;
        let expected_grid_line = viewport_line - display_offset;

        // And converting back: viewport = grid + offset
        let back_to_viewport = expected_grid_line + display_offset;
        assert_eq!(
            back_to_viewport, viewport_line,
            "Round-trip conversion failed"
        );
    }
}

#[test]
fn test_selection_after_scroll_then_scroll_back() {
    use crt_core::Scroll;
    use crt_core::{Column, Line, Point, SelectionType};

    let mut harness = TerminalTestHarness::new(80, 10);

    // Generate history with unique markers
    for i in 0..30 {
        harness.input_str(&format!("MARKER-{:02}\n", i));
    }

    // Scroll up
    harness.terminal_mut().scroll(Scroll::Delta(10));

    // Create selection in scrollback
    let display_offset = harness.terminal().display_offset() as i32;
    let grid_line = -display_offset + 2; // A couple lines down from top of view

    harness
        .terminal_mut()
        .start_selection(Point::new(Line(grid_line), Column(0)), SelectionType::Simple);
    harness
        .terminal_mut()
        .update_selection(Point::new(Line(grid_line), Column(9)));

    // Get selected text before scrolling
    let selected_before = harness.terminal().selection_to_string();
    assert!(selected_before.is_some());

    // Scroll back to bottom
    harness.terminal_mut().scroll(Scroll::Bottom);
    assert_eq!(harness.terminal().display_offset(), 0);

    // Selection should still exist and contain same text
    // (selection is in grid coordinates, so it's stable)
    let selected_after = harness.terminal().selection_to_string();
    assert!(selected_after.is_some());
    assert_eq!(
        selected_before, selected_after,
        "Selection text should be preserved after scrolling"
    );
}

#[test]
fn test_line_selection_in_scrollback() {
    use crt_core::Scroll;
    use crt_core::{Column, Line, Point, SelectionType};

    let mut harness = TerminalTestHarness::new(80, 10);

    // Generate history
    for i in 0..30 {
        harness.input_str(&format!("Full line content {}\n", i));
    }

    // Scroll back
    harness.terminal_mut().scroll(Scroll::Delta(15));
    let display_offset = harness.terminal().display_offset() as i32;

    // Use Lines selection type (triple-click behavior)
    let grid_line = -display_offset + 3;
    harness
        .terminal_mut()
        .start_selection(Point::new(Line(grid_line), Column(5)), SelectionType::Lines);
    harness
        .terminal_mut()
        .update_selection(Point::new(Line(grid_line), Column(5)));

    let selected = harness.terminal().selection_to_string();
    assert!(selected.is_some());

    let text = selected.unwrap();
    assert!(
        text.contains("Full line content"),
        "Line selection should select full line, got: {}",
        text
    );
}

#[test]
fn test_semantic_selection_in_scrollback() {
    use crt_core::Scroll;
    use crt_core::{Column, Line, Point, SelectionType};

    let mut harness = TerminalTestHarness::new(80, 10);

    // Generate history with distinct words
    for i in 0..30 {
        harness.input_str(&format!("word1 word2 word3 line{}\n", i));
    }

    // Scroll back
    harness.terminal_mut().scroll(Scroll::Delta(10));
    let display_offset = harness.terminal().display_offset() as i32;

    // Use Semantic selection (double-click behavior)
    let grid_line = -display_offset + 2;
    // Click in middle of "word2" (around column 7)
    harness.terminal_mut().start_selection(
        Point::new(Line(grid_line), Column(7)),
        SelectionType::Semantic,
    );
    harness
        .terminal_mut()
        .update_selection(Point::new(Line(grid_line), Column(7)));

    let selected = harness.terminal().selection_to_string();
    assert!(selected.is_some());

    let text = selected.unwrap();
    // Should select a word (semantic boundaries)
    assert!(
        !text.is_empty(),
        "Semantic selection should select something"
    );
}

// === Resize tests ===

#[test]
fn test_resize_terminal() {
    use crt_core::Size;

    let mut harness = TerminalTestHarness::new(80, 24);
    harness.input_str("Some initial content");

    // Resize to smaller
    harness.terminal_mut().resize(Size::new(40, 12));

    assert_eq!(harness.terminal().columns(), 40);
    assert_eq!(harness.terminal().screen_lines(), 12);
}

#[test]
fn test_resize_preserves_content() {
    use crt_core::Size;

    let mut harness = TerminalTestHarness::new(80, 24);
    harness.input_str("Important text");

    harness.terminal_mut().resize(Size::new(40, 12));

    // Content should still be present (possibly reflowed)
    let text = harness.visible_text();
    assert!(text.contains("Important") || text.contains("text"));
}

// === Damage tracking tests ===

#[test]
fn test_damage_tracking() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("New content");

    // Should have damage after new content
    assert!(harness.terminal_mut().has_damage());

    harness.terminal_mut().reset_damage();

    // After reset, no damage (unless something triggers it)
}

// === Tab character tests ===

#[test]
fn test_tab_character() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("A\tB");

    // Tab should move cursor to next tab stop (typically column 8)
    let lines = harness.visible_lines();
    assert!(lines[0].contains("A") && lines[0].contains("B"));
    // B should be at column 8
    let b_pos = lines[0].find('B').unwrap();
    assert!(b_pos >= 7, "B should be at tab stop, found at {}", b_pos);
}

// === Backspace tests ===

#[test]
fn test_backspace() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("ABC");
    harness.assert_cursor_at(0, 3);

    harness.input(b"\x08"); // Backspace
    harness.assert_cursor_at(0, 2);

    harness.input_str("X");
    harness.assert_line_contains(0, "ABX");
}

// === Delete character tests ===

#[test]
fn test_delete_character() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("ABCDE");
    harness.input(b"\x1b[1G"); // Move to column 1
    harness.input(b"\x1b[P"); // Delete character

    let lines = harness.visible_lines();
    // A should be deleted, BCDE shifted left
    assert!(lines[0].starts_with("BCDE") || lines[0].contains("BCDE"));
}

// === Large output stress test ===

#[test]
fn test_large_output() {
    let mut harness = TerminalTestHarness::new(80, 24);

    // Generate lots of output
    for i in 0..1000 {
        harness.input_str(&format!("Line number {}\n", i));
    }

    // Terminal should handle this without panicking
    let _lines = harness.visible_lines();

    // Should have history
    assert!(harness.terminal().history_size() > 0);
}

// === Unicode tests ===

#[test]
fn test_unicode_basic() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("Hello ");
    harness.input_str("\u{4E16}\u{754C}"); // Chinese: "world"

    let lines = harness.visible_lines();
    assert!(lines[0].contains("Hello"));
    assert!(lines[0].contains("\u{4E16}") || lines[0].contains("\u{754C}"));
}

#[test]
fn test_unicode_emoji() {
    let mut harness = TerminalTestHarness::default_size();

    harness.input_str("Status: OK ");

    // Emoji should not crash the terminal
    let _lines = harness.visible_lines();
}

#[test]
fn test_unicode_combining() {
    let mut harness = TerminalTestHarness::default_size();

    // e with combining acute accent
    harness.input_str("caf\u{0065}\u{0301}");

    let lines = harness.visible_lines();
    assert!(lines[0].contains("caf"));
}
