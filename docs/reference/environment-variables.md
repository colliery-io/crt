# Environment Variables Reference

---

## CRT_CONFIG_DIR

| Attribute | Value |
|---|---|
| Type | Absolute path string |
| Default | `~/.config/crt` |

Overrides the directory CRT uses to locate `config.toml`, the `themes/` subdirectory, and the `fonts/` subdirectory. Must be an absolute path. If a relative path is supplied, CRT logs a warning and ignores the variable, falling back to `~/.config/crt`.

**Example:**

```sh
CRT_CONFIG_DIR=/opt/myteam/crt-config crt
```

**Use cases:**
- Running multiple CRT configurations side-by-side.
- Isolating config in CI or test environments.
- Pointing at a shared or version-controlled config directory.

---

## CRT_PROFILE

| Attribute | Value |
|---|---|
| Type | Any value (presence is checked, not the value) |
| Default | Unset (profiling disabled) |

Setting this variable to any value enables profiling mode at startup. Profiling collects per-frame timing (total, update, render, present, effects phases), memory samples (RSS, sampled every second), subsystem durations, slow-frame events (>16 ms), and periodic grid snapshots. Output is written to `~/.config/crt/profile-{unix_timestamp}.log`.

Setting `CRT_PROFILE` also changes the default `RUST_LOG` filter from `warn,crt=info` to `warn,crt=debug,crt_renderer=debug,crt_theme=debug,crt_core=debug`, producing verbose logs in the profile file.

Profiling can also be toggled at runtime without a restart via `Cmd+Option+P` or View > Start Profiling in the menu.

**Example:**

```sh
CRT_PROFILE=1 crt
```

**Use cases:**
- Diagnosing frame rate drops or jank.
- Measuring effect performance before filing a bug report.
- Sharing a profile log with developers for analysis.

---

## CRT_BENCHMARK

| Attribute | Value |
|---|---|
| Type | Any value (presence is checked, not the value) |
| Default | Unset |

Enables benchmark mode in the `benchmark_gpu` binary. Not meaningful when passed to the main `crt` binary. Referenced in `benchmark_gpu` usage output.

**Example:**

```sh
CRT_BENCHMARK=1 cargo run --release --bin benchmark_gpu
```

**Use cases:**
- Automated GPU rendering benchmarks in CI.
- Comparing frame timing across builds or hardware.

---

## RUST_LOG

| Attribute | Value |
|---|---|
| Type | `env_logger` filter string |
| Default (normal mode) | `warn,crt=info` |
| Default (profiling mode) | `warn,crt=debug,crt_renderer=debug,crt_theme=debug,crt_core=debug` |

Standard `env_logger` directive. Controls log verbosity for CRT and its crates. When `CRT_PROFILE` is set, CRT automatically uses the debug filter unless `RUST_LOG` is explicitly set, in which case the explicit value takes precedence.

**Example values:**

| Value | Effect |
|---|---|
| `warn,crt=info` | Default: warnings globally, info for CRT crate |
| `warn,crt=debug,crt_renderer=debug,crt_theme=debug,crt_core=debug` | Verbose debug output across all CRT crates |
| `trace` | Maximum verbosity for all crates (very noisy) |
| `crt=debug` | Debug output for the main CRT crate only |
| `error` | Errors only |

**Example:**

```sh
RUST_LOG=crt=debug crt
```

**Use cases:**
- Diagnosing theme loading or config parsing issues.
- Tracing font fallback behavior.
- Investigating rendering or GPU initialization problems.

---

## Summary Table

| Variable | Affects | Required |
|---|---|---|
| `CRT_CONFIG_DIR` | Config file, themes directory, fonts directory | No |
| `CRT_PROFILE` | Enables profiling, switches to debug log level | No |
| `CRT_BENCHMARK` | Enables benchmark mode in `benchmark_gpu` binary | No |
| `RUST_LOG` | Log verbosity filter | No |
