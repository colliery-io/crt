//! Criterion benchmarks for crt-core hot paths.
//!
//! Run with: cargo bench -p crt-core

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use crt_core::{Size, Terminal};

/// Generate realistic terminal output: mixed ASCII with ANSI escape sequences.
fn generate_terminal_output(lines: usize, cols: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(lines * (cols + 20));
    for i in 0..lines {
        // Alternate between plain text and styled text
        if i % 3 == 0 {
            // SGR color sequences + text
            data.extend_from_slice(b"\x1b[32m");
            for c in 0..cols {
                data.push(b'A' + (c % 26) as u8);
            }
            data.extend_from_slice(b"\x1b[0m\r\n");
        } else if i % 3 == 1 {
            // Bold + colored text
            data.extend_from_slice(b"\x1b[1;34m");
            for c in 0..cols {
                data.push(b'a' + (c % 26) as u8);
            }
            data.extend_from_slice(b"\x1b[0m\r\n");
        } else {
            // Plain text
            for c in 0..cols {
                data.push(b'0' + (c % 10) as u8);
            }
            data.extend_from_slice(b"\r\n");
        }
    }
    data
}

/// Benchmark terminal text processing throughput (process_input)
fn bench_process_input(c: &mut Criterion) {
    let mut group = c.benchmark_group("process_input");

    for &(lines, cols) in &[(24, 80), (50, 200), (500, 200)] {
        let data = generate_terminal_output(lines, cols);
        let size = Size::new(cols, lines.min(50)); // Terminal viewport

        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("ansi_text", format!("{}x{}", lines, cols)),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut terminal = Terminal::new(size);
                    terminal.process_input(data);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark all_lines_text() extraction
fn bench_all_lines_text(c: &mut Criterion) {
    let mut group = c.benchmark_group("all_lines_text");

    for &(lines, cols) in &[(24, 80), (50, 200)] {
        let data = generate_terminal_output(lines * 2, cols); // Fill scrollback too
        let size = Size::new(cols, lines);
        let mut terminal = Terminal::new(size);
        terminal.process_input(&data);

        group.bench_with_input(
            BenchmarkId::new("extract", format!("{}x{}", lines, cols)),
            &(),
            |b, _| {
                b.iter(|| {
                    terminal.all_lines_text()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark content hash computation (renderable_content iteration)
fn bench_content_hash(c: &mut Criterion) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    let mut group = c.benchmark_group("content_hash");

    for &(lines, cols) in &[(24, 80), (50, 200)] {
        let data = generate_terminal_output(lines, cols);
        let size = Size::new(cols, lines);
        let mut terminal = Terminal::new(size);
        terminal.process_input(&data);

        group.bench_with_input(
            BenchmarkId::new("hash", format!("{}x{}", lines, cols)),
            &(),
            |b, _| {
                b.iter(|| {
                    let mut hasher = DefaultHasher::new();
                    let content = terminal.renderable_content();
                    hasher.write_i32(content.cursor.point.line.0);
                    hasher.write_usize(content.cursor.point.column.0);
                    for cell in content.display_iter {
                        hasher.write_u32(cell.c as u32);
                    }
                    hasher.finish()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark damage tracking query
fn bench_damage_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("damage_check");

    let size = Size::new(80, 24);
    let mut terminal = Terminal::new(size);
    let data = generate_terminal_output(24, 80);
    terminal.process_input(&data);
    terminal.reset_damage(); // Clear initial damage

    // Type a single character to create partial damage
    terminal.process_input(b"x");

    group.bench_function("has_damage", |b| {
        b.iter(|| terminal.has_damage());
    });

    group.bench_function("damaged_line_set", |b| {
        b.iter(|| terminal.damaged_line_set());
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_process_input,
    bench_all_lines_text,
    bench_content_hash,
    bench_damage_check,
);
criterion_main!(benches);
