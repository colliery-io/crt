# CRT Terminal - Zsh semantic prompt integration
# This script adds OSC 133 markers for command success/fail detection.
# We capture exit code BEFORE sourcing user's zshrc to beat p10k/oh-my-zsh.

# Track whether a command has been executed (don't report exit code on startup)
__crt_cmd_executed=0
# Store exit code immediately (before any other hooks can modify $?)
__crt_last_exit=0

# This hook runs FIRST to capture exit code before p10k/oh-my-zsh touch it
# MUST return the same exit code so p10k can still display it
__crt_precmd_first() {
    __crt_last_exit=$?
    # Re-forward the exit code to subsequent hooks (like p10k)
    return $__crt_last_exit
}

# Track command execution
__crt_preexec() {
    __crt_cmd_executed=1
    # B = command start (user pressed enter)
    printf '\e]133;B\a'
}

# Load zsh hook system and register FIRST hooks before anything else
autoload -Uz add-zsh-hook
add-zsh-hook precmd __crt_precmd_first
add-zsh-hook preexec __crt_preexec

# Restore original ZDOTDIR and source user's zshrc
if [[ -n "$CRT_ORIGINAL_ZDOTDIR" ]]; then
    ZDOTDIR="$CRT_ORIGINAL_ZDOTDIR"
    unset CRT_ORIGINAL_ZDOTDIR
else
    unset ZDOTDIR
fi

# Source user's existing zshrc (loads oh-my-zsh, p10k, etc.)
[ -f ~/.zshrc ] && source ~/.zshrc

# This hook runs LAST to emit OSC 133 sequences after prompt is ready
__crt_precmd_last() {
    # Only send exit code if a command was actually executed
    if [[ $__crt_cmd_executed -eq 1 ]]; then
        # D = command finished with exit code
        printf '\e]133;D;%d\a' "$__crt_last_exit"
        __crt_cmd_executed=0
    fi
    # A = prompt start
    printf '\e]133;A\a'
}

# Register last hook (after user's zshrc so it runs after p10k)
add-zsh-hook precmd __crt_precmd_last
