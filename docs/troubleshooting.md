# Troubleshooting

This guide covers common issues and their solutions.

## Shell Integration & Reactive Themes

CRT supports reactive themes that respond to terminal events like command success/failure. This requires shell integration via OSC 133 semantic prompts.

### Powerlevel10k / Oh-My-Zsh Users

If you use Powerlevel10k (p10k) with Oh-My-Zsh, CRT's default shell integration can cause issues:

- **PATH ordering problems** - paths from `.zshenv` (like cargo) end up at the end
- **Reactive themes not triggering** - hook conflicts with p10k

**Solution:** Use p10k's native OSC 133 support instead of CRT's:

1. **Disable CRT's semantic prompts** in `~/.config/crt/config.toml`:
   ```toml
   [shell]
   semantic_prompts = false
   ```

2. **Enable p10k's shell integration** by adding to `~/.p10k.zsh`:
   ```zsh
   typeset -g POWERLEVEL9K_TERM_SHELL_INTEGRATION=true
   ```

3. **Clear p10k's instant prompt cache**:
   ```bash
   rm -f ~/.cache/p10k-instant-prompt-*.zsh
   ```

4. Restart CRT

### Reactive Themes Not Working

If reactive themes (like `nyancat-responsive` or `robco-reactive`) don't respond to command success/failure:

1. **Check shell integration is enabled** - either via CRT's `semantic_prompts = true` or p10k's `POWERLEVEL9K_TERM_SHELL_INTEGRATION=true`

2. **Test OSC 133 is working** - launch CRT from terminal with debug logging:
   ```bash
   RUST_LOG=debug /Applications/crt.app/Contents/MacOS/crt
   ```
   Look for `TerminalEvent::CommandSuccess` or `TerminalEvent::CommandFail` in the terminal output where you launched CRT (not inside CRT itself).

3. **Verify theme has event handlers** - check your theme CSS for `:terminal::on-command-success` and `:terminal::on-command-fail` selectors

### Shell Starts with Exit Code 1

If your shell prompt shows a non-zero exit code immediately on startup (before running any commands):

This usually means something in your shell initialization is returning an error. Common causes:

- **`unalias` commands** for aliases that don't exist - add `|| true`:
  ```zsh
  unalias foo 2>/dev/null || true
  ```

- **Missing commands** in plugins - disable unused oh-my-zsh plugins (e.g., `systemd`, `ubuntu` on macOS)

- **Failed conditionals** - ensure the last command in your `.zshrc` returns 0

To debug, run:
```bash
zsh -xc 'source ~/.zshrc' 2>&1 | grep -i error
```

## Sprite & Asset Issues

### Sprite Not Loading

If sprites don't appear or you see path errors in logs:

1. **Check the sprite path** - paths in CSS are relative to the theme directory:
   ```css
   /* If sprite is at ~/.config/crt/themes/mytheme/sprites/cat.png */
   --sprite-path: "sprites/cat.png";
   ```

2. **Verify assets were installed** - check `~/.config/crt/themes/` contains sprite subdirectories

3. **Check file permissions** - sprite files must be readable

### Theme Hot Reload Not Working

If editing theme CSS doesn't update the terminal:

1. **Save the file** - some editors use atomic saves that the watcher may miss
2. **Check the file path** - hot reload only works for themes in `~/.config/crt/themes/`
3. **Restart CRT** if hot reload seems stuck

## Performance Issues

### High CPU Usage

1. **Disable unused effects** - complex effects like `matrix` or `particles` use GPU
2. **Reduce effect intensity** - lower particle counts, star counts, etc.
3. **Check for animation loops** - infinite sprite animations consume resources

### Slow Startup

1. **Check shell initialization** - CRT's startup time includes shell init
2. **Disable p10k instant prompt** temporarily to isolate the issue
3. **Review oh-my-zsh plugins** - many plugins slow down shell startup

## macOS-Specific Issues

### App Killed After Manual Binary Update

If you manually replace the binary in `/Applications/crt.app/Contents/MacOS/crt`, macOS may kill it due to invalid code signature.

**Solution:** Re-sign the app:
```bash
sudo codesign --force --deep --sign - /Applications/crt.app
```

### Accessibility Permissions

Some features may require accessibility permissions. Grant them in:
System Preferences > Security & Privacy > Privacy > Accessibility

## Getting Help

If your issue isn't covered here:

1. **Check logs** - launch CRT from terminal with `RUST_LOG=debug` (logs appear in the terminal where you launched, not inside CRT):
   ```bash
   RUST_LOG=debug /Applications/crt.app/Contents/MacOS/crt
   ```

2. **Search existing issues** - https://github.com/colliery-io/crt/issues

3. **Open an issue** - include relevant logs, your config, and steps to reproduce
