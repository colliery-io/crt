# CRT Terminal - Zsh environment integration
# Source user's .zshenv if it exists (critical for PATH, cargo, etc.)

# Restore original ZDOTDIR for .zshenv lookup
if [[ -n "$CRT_ORIGINAL_ZDOTDIR" ]]; then
    _crt_user_zshenv="$CRT_ORIGINAL_ZDOTDIR/.zshenv"
else
    _crt_user_zshenv="$HOME/.zshenv"
fi

# Source user's .zshenv
[[ -f "$_crt_user_zshenv" ]] && source "$_crt_user_zshenv"
unset _crt_user_zshenv
