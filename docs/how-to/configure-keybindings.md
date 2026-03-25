# How to Configure Keybindings

Customize CRT Terminal's keyboard shortcuts by editing the `[[keybindings.bindings]]` section of your `config.toml`.

## Important: Custom Bindings Replace Defaults

When you define any `[[keybindings.bindings]]` entries, they **replace the entire default set**. If you add one custom binding, all defaults are gone. To keep the defaults while adding your own, copy the full default list and append your additions.

## Keybinding Format

Each binding is a TOML array entry with three fields:

```toml
[[keybindings.bindings]]
key = "t"              # Key name
mods = ["super"]       # Modifier keys (optional, default: [])
action = "new_tab"     # Action to perform
```

### Key Names

Use lowercase letter names for letter keys (`"t"`, `"w"`, `"q"`), number strings for digits (`"1"`, `"9"`), and special names for other keys:

| Key Name | Physical Key |
|----------|-------------|
| `"equal"` | `=` / `+` |
| `"minus"` | `-` / `_` |
| `"["` | `[` / `{` |
| `"]"` | `]` / `}` |
| `"space"` | Space bar |
| `"F1"` - `"F12"` | Function keys |

### Modifier Names

| Modifier | macOS | Linux |
|----------|-------|-------|
| `"super"` | Cmd | Super/Meta |
| `"shift"` | Shift | Shift |
| `"ctrl"` | Control | Control |
| `"alt"` | Option | Alt |

### Available Actions

| Action | Description |
|--------|-------------|
| `new_tab` | Open a new tab |
| `close_tab` | Close the current tab |
| `next_tab` | Switch to the next tab |
| `prev_tab` | Switch to the previous tab |
| `select_tab1` - `select_tab9` | Switch to tab 1-9 |
| `increase_font_size` | Zoom in |
| `decrease_font_size` | Zoom out |
| `reset_font_size` | Reset zoom to 100% |
| `toggle_fullscreen` | Toggle fullscreen mode |
| `copy` | Copy selection to clipboard |
| `paste` | Paste from clipboard |
| `quit` | Quit CRT |

## Complete Default Keybindings

Copy this entire block into your `config.toml` as a starting point, then modify or add entries:

```toml
# Tab management
[[keybindings.bindings]]
key = "t"
mods = ["super"]
action = "new_tab"

[[keybindings.bindings]]
key = "w"
mods = ["super"]
action = "close_tab"

[[keybindings.bindings]]
key = "["
mods = ["super", "shift"]
action = "prev_tab"

[[keybindings.bindings]]
key = "]"
mods = ["super", "shift"]
action = "next_tab"

# Tab selection (Cmd+1 through Cmd+9)
[[keybindings.bindings]]
key = "1"
mods = ["super"]
action = "select_tab1"

[[keybindings.bindings]]
key = "2"
mods = ["super"]
action = "select_tab2"

[[keybindings.bindings]]
key = "3"
mods = ["super"]
action = "select_tab3"

[[keybindings.bindings]]
key = "4"
mods = ["super"]
action = "select_tab4"

[[keybindings.bindings]]
key = "5"
mods = ["super"]
action = "select_tab5"

[[keybindings.bindings]]
key = "6"
mods = ["super"]
action = "select_tab6"

[[keybindings.bindings]]
key = "7"
mods = ["super"]
action = "select_tab7"

[[keybindings.bindings]]
key = "8"
mods = ["super"]
action = "select_tab8"

[[keybindings.bindings]]
key = "9"
mods = ["super"]
action = "select_tab9"

# Font size
[[keybindings.bindings]]
key = "equal"
mods = ["super"]
action = "increase_font_size"

[[keybindings.bindings]]
key = "minus"
mods = ["super"]
action = "decrease_font_size"

[[keybindings.bindings]]
key = "0"
mods = ["super"]
action = "reset_font_size"

# Clipboard
[[keybindings.bindings]]
key = "c"
mods = ["super"]
action = "copy"

[[keybindings.bindings]]
key = "v"
mods = ["super"]
action = "paste"

# Application
[[keybindings.bindings]]
key = "q"
mods = ["super"]
action = "quit"
```

## Common Customizations

### Remap Quit to Ctrl+Q

Replace the Cmd+Q quit binding:

```toml
[[keybindings.bindings]]
key = "q"
mods = ["ctrl"]
action = "quit"
```

### Add Function Key for Fullscreen

```toml
[[keybindings.bindings]]
key = "F11"
action = "toggle_fullscreen"
```

### Use Ctrl+Tab / Ctrl+Shift+Tab for Tab Switching

```toml
[[keybindings.bindings]]
key = "tab"
mods = ["ctrl"]
action = "next_tab"

[[keybindings.bindings]]
key = "tab"
mods = ["ctrl", "shift"]
action = "prev_tab"
```

### Minimal Config with Only What You Need

If you only use a few shortcuts:

```toml
[[keybindings.bindings]]
key = "t"
mods = ["super"]
action = "new_tab"

[[keybindings.bindings]]
key = "w"
mods = ["super"]
action = "close_tab"

[[keybindings.bindings]]
key = "c"
mods = ["super"]
action = "copy"

[[keybindings.bindings]]
key = "v"
mods = ["super"]
action = "paste"
```

## Hardcoded Shortcuts

The following shortcuts are built into CRT and cannot be changed via config:

| Shortcut | Action |
|----------|--------|
| Cmd+N | New window |
| Cmd+Shift+W | Close window |
| Cmd+Shift+R | Rename window |
| Cmd+F | Open search |
| Cmd+A | Select all |
| Cmd+K | Clear scrollback |
| Cmd+Option+P | Toggle profiling |
| Ctrl+Cmd+F | Toggle fullscreen |
| Shift+PageUp/PageDown | Scroll history |
| Shift+Home/End | Jump to top/bottom of scrollback |
| Cmd+Click | Open URL |
| Double-click tab | Rename tab |
| Right-click | Context menu |

## Troubleshooting

**Binding not working?**
- Check that the action name is spelled correctly (use snake_case)
- Verify modifier names are lowercase strings in an array
- Remember that custom bindings replace all defaults

**Invalid action name?**
CRT will show a config error toast if a binding has an unrecognized action. Check the terminal log for details.

## See Also

- [Configuration Reference](../reference/configuration.md) for all config options
- [Keyboard Shortcuts Reference](../reference/keyboard-shortcuts.md) for the complete shortcut list
