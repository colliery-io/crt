# How to Set Up Reactive Themes

Reactive themes let your terminal's appearance respond to shell events: a command succeeds, a command fails, the terminal bell fires, or the window gains or loses focus. This guide walks through enabling the feature, wiring up your shell, and writing CSS event selectors.

---

## What are reactive themes?

CRT themes can define CSS rules under event selectors such as `::on-command-success` or `::on-command-fail`. When the matching event fires, CRT temporarily overrides properties from the base theme — changing colours, swapping a sprite, or altering glow intensity — and then reverts after a configurable duration.

Events are signalled from your shell using **OSC 133** escape sequences, a standard protocol for semantic shell integration. CRT listens for these sequences and triggers the corresponding theme selectors.

The built-in `robco-reactive` theme is a good example to study: it swaps Vault Boy sprite frames on success, failure, and bell events.

---

## Step 1: Enable semantic prompts in config

Open `~/.config/crt/config.toml` and add or uncomment:

```toml
[shell]
semantic_prompts = true
```

This tells CRT to pay attention to OSC 133 sequences emitted by your shell. Without this setting the sequences are ignored even if your shell sends them.

Restart CRT (or open a new window) after editing the config.

---

## Step 2: Wire up your shell

### Option A: Use the built-in integration scripts (easiest)

CRT ships integration scripts at `~/.config/crt/shell/`. These handle OSC 133 correctly and work alongside existing prompt frameworks.

**Zsh:** The zsh integration uses a custom `ZDOTDIR`. Add to `~/.zshrc`:

```sh
# Point ZDOTDIR at CRT's scripts; they will source your real ~/.zshrc
# Only activate when running inside CRT
if [[ "$TERM_PROGRAM" == "crt" ]]; then
    export CRT_ORIGINAL_ZDOTDIR="$ZDOTDIR"
    export ZDOTDIR="$HOME/.config/crt/shell/crt-zsh-init"
fi
```

Alternatively, launch CRT with the shell argument:

```toml
# ~/.config/crt/config.toml
[shell]
program = "/bin/zsh"
args = ["--rcs"]
semantic_prompts = true
```

And set `ZDOTDIR` in your shell launch configuration so CRT's `.zshenv` is sourced first.

**Bash:** Source the bash init script from your `~/.bashrc`:

```sh
# Only inside CRT
if [[ "$TERM_PROGRAM" == "crt" ]]; then
    source "$HOME/.config/crt/shell/crt-bash-init"
fi
```

### Option B: Manual Bash integration

Add these two lines to `~/.bashrc`:

```sh
# OSC 133 A: prompt start
PS1='\[\e]133;A\a\]'"$PS1"

# OSC 133 D: command finished with exit code
PROMPT_COMMAND='printf "\033]133;D;$?\007"'"${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
```

The order matters: `PROMPT_COMMAND` must emit `D` before `PS1` emits `A`.

### Option C: Manual Zsh integration

Add these hooks to `~/.zshrc`:

```zsh
autoload -Uz add-zsh-hook

# Emit A (prompt start) and D (command done with exit code) before each prompt
_crt_precmd() {
    local code=$?
    printf '\e]133;D;%d\a' "$code"
    printf '\e]133;A\a'
}

# Emit C (command start) when user executes a command
_crt_preexec() {
    printf '\e]133;C\a'
}

add-zsh-hook precmd _crt_precmd
add-zsh-hook preexec _crt_preexec
```

---

## Step 3: Powerlevel10k compatibility

Powerlevel10k has its own shell integration mechanism that conflicts with manual OSC 133 hooks. Use the native p10k setting instead of adding your own hooks:

```zsh
# In ~/.zshrc, before `source $ZSH/oh-my-zsh.sh` or p10k initialisation
POWERLEVEL9K_TERM_SHELL_INTEGRATION=true
```

Then **disable** CRT's duplicate semantic prompt handling so the sequences are not doubled:

```toml
# ~/.config/crt/config.toml
[shell]
semantic_prompts = false
```

CRT will still receive and act on the OSC 133 sequences that p10k emits natively.

### Oh My Zsh (without Powerlevel10k)

Oh My Zsh works fine with the manual zsh hooks above. Add the `_crt_precmd` and `_crt_preexec` hooks after `source $ZSH/oh-my-zsh.sh` in your `~/.zshrc`.

---

## Step 4: Verify OSC 133 is working

To confirm events are reaching CRT, use the `robco-reactive` theme temporarily:

```toml
# ~/.config/crt/config.toml
[theme]
name = "robco-reactive"

[shell]
semantic_prompts = true
```

Restart CRT. Run a command that exits 0 (`ls`) and one that fails (`ls /nonexistent`). If the Vault Boy sprite changes briefly, OSC 133 is working.

For a minimal test without a graphical sprite, set `RUST_LOG=debug` before launching:

```sh
RUST_LOG=debug crt 2>&1 | grep -i "osc\|semantic\|event"
```

Look for log lines mentioning `OSC 133` or `semantic prompt` marker detection.

---

## Step 5: Write event selectors in your theme

Event selectors are pseudo-elements appended to `:terminal`. They inherit the base theme and override only the properties you specify.

### Available selectors

| Selector | Trigger |
|---|---|
| `::on-bell` | Terminal bell (`\a`) received |
| `::on-command-success` | Last command exited with code 0 |
| `::on-command-fail` | Last command exited with non-zero code |
| `::on-focus` | Window gained focus |
| `::on-blur` | Window lost focus |

### The `--duration` property

Every event selector should include `--duration` to control how long the override lasts before the base theme resumes:

```css
:terminal::on-command-fail {
    --duration: 3000ms;   /* revert after 3 seconds */
    color: #ff5555;
    text-shadow: 0 0 20px rgba(255, 85, 85, 0.8);
}
```

Set `--duration: 0ms` for a persistent override that stays until the next event clears it:

```css
:terminal::on-blur {
    --duration: 0ms;   /* stays until on-focus fires */
    color: #666666;
    text-shadow: none;
}
```

### A minimal reactive theme

```css
:terminal {
    color: #e0e0e0;
    background: #1a1a1a;
    text-shadow: none;
}

:terminal::on-command-success {
    --duration: 600ms;
    text-shadow: 0 0 10px rgba(80, 250, 123, 0.7);
}

:terminal::on-command-fail {
    --duration: 2000ms;
    text-shadow: 0 0 15px rgba(255, 85, 85, 0.8);
    color: #ff8888;
}

:terminal::on-bell {
    --duration: 300ms;
    background: #2a1a1a;
}

:terminal::on-blur {
    --duration: 0ms;
    color: #888888;
}

:terminal::on-focus {
    --duration: 0ms;
    color: #e0e0e0;
}
```

Save this as `~/.config/crt/themes/my-reactive.css`, then set `name = "my-reactive"` in `config.toml`.

---

## Troubleshooting

**Events fire on startup before any command is run.**
The shell integration scripts include a guard (`__crt_cmd_executed`) that suppresses the first `D` sequence. If you wrote manual hooks, add the same guard:

```zsh
_crt_cmd_executed=0
_crt_precmd() {
    local code=$?
    if [[ $_crt_cmd_executed -eq 1 ]]; then
        printf '\e]133;D;%d\a' "$code"
        _crt_cmd_executed=0
    fi
    printf '\e]133;A\a'
}
_crt_preexec() { _crt_cmd_executed=1 }
```

**Reactive theme is not changing anything.**
1. Confirm `semantic_prompts = true` is set in `[shell]` in `config.toml`.
2. Confirm your shell is sending OSC 133 sequences (use the debug log check above).
3. Confirm the theme CSS file contains event selectors with `--duration`.

**Powerlevel10k shows wrong exit codes after adding hooks.**
Use `POWERLEVEL9K_TERM_SHELL_INTEGRATION=true` and remove manual hooks. See Step 3.

**Fish shell.**
Fish does not support OSC 133 natively via a simple `precmd` hook. A workaround is to add to `~/.config/fish/config.fish`:

```fish
function __crt_postexec --on-event fish_postexec
    printf '\e]133;D;%d\a' $status
end

function __crt_prompt --on-event fish_prompt
    printf '\e]133;A\a'
end
```

---

## Related guides

- [How to Create a Custom Theme](./create-custom-theme.md)
- [Theme CSS Properties Reference](../reference/theme-css-properties.md)
