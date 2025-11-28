//! Shell integration tests
//!
//! Tests real shell interaction via PTY.
//! These tests spawn actual shell processes.

mod common;

use common::ShellTestHarness;
use std::time::Duration;

// === Basic shell tests ===

#[test]
fn test_shell_spawn() {
    let harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    // Shell should be running
    assert_eq!(harness.shell().terminal().columns(), 80);
    assert_eq!(harness.shell().terminal().screen_lines(), 24);
}

#[test]
fn test_shell_echo() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    // Wait for shell to start
    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Send echo command
    harness.send("echo TESTOUTPUT123\n");

    // Wait for output
    harness.wait_for_text("TESTOUTPUT123")
        .expect("Should see echo output");
}

#[test]
fn test_shell_multiple_commands() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Execute multiple commands
    harness.send("echo first\n");
    harness.wait_for_text("first").expect("Should see first");

    harness.send("echo second\n");
    harness.wait_for_text("second").expect("Should see second");

    harness.send("echo third\n");
    harness.wait_for_text("third").expect("Should see third");
}

#[test]
fn test_shell_pwd() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    harness.send("pwd\n");

    // Should output some path (starting with /)
    harness.wait_for_text("/").expect("Should see a path");
}

// === Environment tests ===

#[test]
fn test_shell_term_env() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    harness.send("echo $TERM\n");

    // Should be set to xterm-256color
    harness.wait_for_text("xterm-256color")
        .expect("TERM should be xterm-256color");
}

#[test]
fn test_shell_colorterm_env() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    harness.send("echo $COLORTERM\n");

    // Should be set to truecolor
    harness.wait_for_text("truecolor")
        .expect("COLORTERM should be truecolor");
}

// === Resize tests ===

#[test]
fn test_shell_resize() {
    use crt_core::Size;

    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Resize terminal
    harness.shell_mut().resize(Size::new(120, 40));

    // Check terminal reflects new size
    assert_eq!(harness.shell().terminal().columns(), 120);
    assert_eq!(harness.shell().terminal().screen_lines(), 40);

    // Shell should still work after resize
    harness.send("echo resized\n");
    harness.wait_for_text("resized").expect("Should work after resize");
}

// === Exit status tests ===

#[test]
fn test_shell_exit_status() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Run command that succeeds
    harness.send("true; echo $?\n");
    harness.wait_for_text("0").expect("Exit status should be 0");

    // Run command that fails
    harness.send("false; echo $?\n");
    harness.wait_for_text("1").expect("Exit status should be 1");
}

// === Control character tests ===

#[test]
fn test_shell_ctrl_c() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Start a command that we'll interrupt
    harness.send("echo before; ");

    // Send Ctrl-C (SIGINT)
    harness.send_bytes(&[0x03]); // ETX = Ctrl-C

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Should still be able to run commands
    harness.send("echo after\n");
    harness.wait_for_text("after").expect("Shell should still work after Ctrl-C");
}

#[test]
fn test_shell_ctrl_d_partial() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // With text on the line, Ctrl-D should not exit
    harness.send("echo partial");
    harness.send_bytes(&[0x04]); // EOT = Ctrl-D

    std::thread::sleep(Duration::from_millis(100));

    // Shell should still be running
    harness.send("\necho stillhere\n");
    harness.wait_for_text("stillhere").expect("Shell should still be running");
}

// === History tests ===

#[test]
fn test_terminal_history() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Generate output to create history
    for i in 0..50 {
        harness.send(&format!("echo line{}\n", i));
        std::thread::sleep(Duration::from_millis(20));
        harness.process_output();
    }

    // Wait for all output
    harness.wait_for_text("line49").expect("Should see last line");

    // Terminal should have history
    let history_size = harness.shell().terminal().history_size();
    assert!(history_size > 0, "Should have history, got {}", history_size);
}

#[test]
fn test_terminal_scroll() {
    use crt_core::Scroll;

    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Generate lots of output
    for i in 0..50 {
        harness.send(&format!("echo scroll{}\n", i));
        std::thread::sleep(Duration::from_millis(20));
        harness.process_output();
    }

    std::thread::sleep(Duration::from_millis(200));
    harness.process_output();

    // Should be at bottom initially
    assert!(!harness.shell().terminal().is_scrolled_back());

    // Scroll up
    harness.shell_mut().scroll(Scroll::Delta(10));
    assert!(harness.shell().terminal().is_scrolled_back());

    // Scroll to bottom
    harness.shell_mut().scroll_to_bottom();
    assert!(!harness.shell().terminal().is_scrolled_back());
}

// === Selection tests with shell ===

#[test]
fn test_selection_shell_output() {
    use crt_core::{Point, Column, Line, SelectionType};

    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    harness.send("echo SelectableText\n");
    harness.wait_for_text("SelectableText").expect("Should see output");

    // Try to select text
    harness.shell_mut().start_selection(
        Point::new(Line(0), Column(0)),
        SelectionType::Simple
    );
    harness.shell_mut().update_selection(
        Point::new(Line(0), Column(20))
    );

    assert!(harness.shell().has_selection());

    // Get selected text
    let selected = harness.shell().selection_to_string();
    assert!(selected.is_some());
}

// === Cat test (tests buffering) ===

#[test]
fn test_shell_cat() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Echo some text through cat
    harness.send("echo 'cat test line' | cat\n");

    harness.wait_for_text("cat test line")
        .expect("Should see cat output");
}

// === Pipe test ===

#[test]
fn test_shell_pipe() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    harness.send("echo 'hello world' | tr 'a-z' 'A-Z'\n");

    harness.wait_for_text("HELLO WORLD")
        .expect("Should see transformed output");
}

// === Long line test ===

#[test]
fn test_shell_long_line() {
    let mut harness = ShellTestHarness::with_shell(80, 24, "/bin/sh")
        .expect("Failed to spawn shell");

    std::thread::sleep(Duration::from_millis(100));
    harness.process_output();

    // Create a line longer than terminal width
    let long_text = "A".repeat(200);
    harness.send(&format!("echo {}\n", long_text));

    std::thread::sleep(Duration::from_millis(200));
    harness.process_output();

    // Should have content (may be wrapped)
    let content = harness.all_content();
    assert!(content.contains("AAAA"), "Should contain the long line");
}
