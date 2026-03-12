//! Criterion benchmarks for theme CSS parsing.
//!
//! Run with: cargo bench -p crt-theme

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use crt_theme::parser::parse_theme;

const MINIMAL_THEME: &str = r#"
:root {
    --foreground: #e0e0e0;
    --background: #1a1a2e;
    --cursor-color: #00ff41;
}
"#;

const MEDIUM_THEME: &str = r#"
:root {
    --foreground: #e0e0e0;
    --background: linear-gradient(180deg, #0a0a1a, #1a1a2e);
    --cursor-color: #00ff41;
    --cursor-shape: beam;
    --selection-bg: rgba(0, 255, 65, 0.3);
    --selection-fg: #ffffff;
    --text-shadow: 0 0 8px rgba(0, 255, 65, 0.6);
    --font-line-height: 1.4;

    --black: #1a1a2e;
    --red: #ff0055;
    --green: #00ff41;
    --yellow: #f0e68c;
    --blue: #00bfff;
    --magenta: #ff00ff;
    --cyan: #00ffff;
    --white: #e0e0e0;
    --bright-black: #444466;
    --bright-red: #ff3377;
    --bright-green: #33ff66;
    --bright-yellow: #ffff88;
    --bright-blue: #33ccff;
    --bright-magenta: #ff33ff;
    --bright-cyan: #33ffff;
    --bright-white: #ffffff;

    --highlight-match-bg: rgba(255, 255, 0, 0.3);
    --highlight-match-fg: #ffffff;
    --highlight-focused-bg: rgba(255, 165, 0, 0.5);
    --highlight-focused-fg: #ffffff;
}
"#;

const FULL_THEME: &str = r#"
:root {
    --foreground: #e0e0e0;
    --background: linear-gradient(180deg, #0a0a1a, #1a1a2e);
    --cursor-color: #00ff41;
    --cursor-shape: beam;
    --selection-bg: rgba(0, 255, 65, 0.3);
    --selection-fg: #ffffff;
    --text-shadow: 0 0 8px rgba(0, 255, 65, 0.6);
    --font-line-height: 1.4;

    --black: #1a1a2e;
    --red: #ff0055;
    --green: #00ff41;
    --yellow: #f0e68c;
    --blue: #00bfff;
    --magenta: #ff00ff;
    --cyan: #00ffff;
    --white: #e0e0e0;
    --bright-black: #444466;
    --bright-red: #ff3377;
    --bright-green: #33ff66;
    --bright-yellow: #ffff88;
    --bright-blue: #33ccff;
    --bright-magenta: #ff33ff;
    --bright-cyan: #33ffff;
    --bright-white: #ffffff;

    --highlight-match-bg: rgba(255, 255, 0, 0.3);
    --highlight-match-fg: #ffffff;
    --highlight-focused-bg: rgba(255, 165, 0, 0.5);
    --highlight-focused-fg: #ffffff;

    --crt-scanlines: 0.15;
    --crt-curvature: 0.02;
    --crt-vignette: 0.3;
    --crt-flicker: 0.03;
    --crt-glow: 0.4;
}

::on-bell {
    --flash-color: rgba(255, 0, 0, 0.3);
    --flash-intensity: 0.5;
    duration: 300ms;
}

::on-command-success {
    --flash-color: rgba(0, 255, 0, 0.15);
    --flash-intensity: 0.3;
    duration: 200ms;
}

::on-command-fail {
    --flash-color: rgba(255, 0, 0, 0.2);
    --flash-intensity: 0.4;
    duration: 500ms;
}
"#;

fn bench_parse_theme(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_theme");

    group.bench_with_input(
        BenchmarkId::new("css", "minimal"),
        &MINIMAL_THEME,
        |b, css| {
            b.iter(|| parse_theme(css).unwrap());
        },
    );

    group.bench_with_input(
        BenchmarkId::new("css", "medium"),
        &MEDIUM_THEME,
        |b, css| {
            b.iter(|| parse_theme(css).unwrap());
        },
    );

    group.bench_with_input(
        BenchmarkId::new("css", "full"),
        &FULL_THEME,
        |b, css| {
            b.iter(|| parse_theme(css).unwrap());
        },
    );

    group.finish();
}

criterion_group!(benches, bench_parse_theme);
criterion_main!(benches);
