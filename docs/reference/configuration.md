# Configuration Reference

**File location:** `~/.config/crt/config.toml`

The config directory can be overridden via the `CRT_CONFIG_DIR` environment variable (must be an absolute path). If the config file is absent, CRT starts with all defaults. If the file contains a parse error, CRT logs a warning and falls back to all defaults. Unknown fields are silently ignored. Partial configs use defaults for every omitted field; fields within a section that are omitted also use their defaults.

---

## [shell]

| Field | Type | Default | Description |
|---|---|---|---|
| `program` | `string` (optional) | user's login shell, then `/bin/zsh` (macOS) or `/bin/bash` (Linux) | Absolute path to the shell binary to launch. |
| `args` | `string[]` | `[]` | Arguments passed to the shell. |
| `working_directory` | `string` (optional) | user's home directory | Starting working directory for the shell. |
| `semantic_prompts` | `bool` | `false` | Inject OSC 133 shell hooks for command success/failure detection. Not required when using starship, oh-my-zsh, or p10k, which emit OSC 133 natively. |

---

## [font]

| Field | Type | Default | Description |
|---|---|---|---|
| `family` | `string[]` | `["MesloLGS NF", "JetBrains Mono", "Fira Code", "SF Mono", "Menlo"]` | Font families in preference order. The first family found on the system is used. Custom fonts can be placed in `~/.config/crt/fonts/`, which is searched before system fonts. Falls back to an embedded font if no listed family is found. |
| `size` | `float` | `14.0` | Base font size in points. |
| `line_height` | `float` | `1.5` | Line height multiplier. `1.0` = no extra spacing; `1.5` = 50% additional vertical space between lines. |

---

## [window]

| Field | Type | Default | Description |
|---|---|---|---|
| `columns` | `integer` | `80` | Initial terminal width in character columns. |
| `rows` | `integer` | `24` | Initial terminal height in character rows. |
| `title` | `string` | `"CRT Terminal"` | Window title bar text. |
| `fullscreen` | `bool` | `false` | Start in fullscreen mode. |

---

## [theme]

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | `string` | `"synthwave"` (release builds); `"nyancat"` (debug builds) | Theme name to load. CRT looks for `~/.config/crt/themes/{name}.css`. All 19 built-in themes are pre-installed in that directory. Custom themes can be added as additional `.css` files. |

---

## [cursor]

| Field | Type | Default | Description |
|---|---|---|---|
| `style` | `"block"` \| `"bar"` \| `"underline"` | `"block"` | Cursor shape. |
| `blink` | `bool` | `true` | Whether the cursor blinks. |
| `blink_interval_ms` | `integer` | `530` | Blink period in milliseconds. |

---

## [bell]

| Field | Type | Default | Description |
|---|---|---|---|
| `visual` | `bool` | `true` | Enable visual bell (screen flash on terminal bell character `\a`). |
| `flash_duration_ms` | `integer` | `100` | Duration of the visual flash in milliseconds. |
| `flash_intensity` | `float` | `0.3` | Intensity of the flash overlay. Range: `0.0` (invisible) to `1.0` (full white). |

---

## [[keybindings.bindings]]

Each entry in the `bindings` array is a table with three fields:

| Field | Type | Required | Description |
|---|---|---|---|
| `key` | `string` | yes | Key name. Single character keys are specified literally (`"t"`, `"w"`, `"c"`). Special keys use names: `"equal"`, `"minus"`, `"0"`–`"9"`, `"[`"`, `"]"`, `"F1"`–`"F12"`. |
| `mods` | `string[]` | no (default `[]`) | Modifier keys. Valid values: `"super"` (Cmd on macOS, Win on Linux), `"shift"`, `"ctrl"`, `"alt"`. |
| `action` | `string` | yes | Action to perform. See table below. |

### Available Actions

| Action | Description |
|---|---|
| `new_tab` | Open a new tab. |
| `close_tab` | Close the active tab (closes the window if only one tab is open). |
| `next_tab` | Switch to the next tab. |
| `prev_tab` | Switch to the previous tab. |
| `select_tab1` – `select_tab9` | Switch directly to tab 1 through 9 by position. |
| `increase_font_size` | Increase font size by one step. |
| `decrease_font_size` | Decrease font size by one step. |
| `reset_font_size` | Reset font size to the value in `[font].size`. |
| `toggle_fullscreen` | Toggle fullscreen mode. |
| `copy` | Copy selected text to the clipboard. |
| `paste` | Paste from the clipboard. |
| `quit` | Quit the application. |

### Keybindings Replacement Behavior

When a `[[keybindings.bindings]]` array is present in `config.toml`, it **replaces the entire default binding list**. There is no merge. To keep all default bindings while adding one more, reproduce all defaults plus the new entry.

### Default Keybindings

| Shortcut | Action |
|---|---|
| `Cmd+T` | `new_tab` |
| `Cmd+W` | `close_tab` |
| `Cmd+Shift+[` | `prev_tab` |
| `Cmd+Shift+]` | `next_tab` |
| `Cmd+1` – `Cmd+9` | `select_tab1` – `select_tab9` |
| `Cmd+=` | `increase_font_size` |
| `Cmd+-` | `decrease_font_size` |
| `Cmd+0` | `reset_font_size` |
| `Cmd+C` | `copy` |
| `Cmd+V` | `paste` |
| `Cmd+Q` | `quit` |

---

## Complete Example config.toml

```toml
# ~/.config/crt/config.toml
# All fields shown with their default values.
# Uncomment and modify any setting you want to change.

[shell]
# program = "/bin/zsh"
# args = ["-l"]
# working_directory = "/Users/you"
# semantic_prompts = false

[font]
family = [
    "MesloLGS NF",
    "JetBrains Mono",
    "Fira Code",
    "SF Mono",
    "Menlo",
]
size = 14.0
line_height = 1.5

[window]
columns = 80
rows = 24
title = "CRT Terminal"
fullscreen = false

[theme]
name = "synthwave"

[cursor]
style = "block"
blink = true
blink_interval_ms = 530

[bell]
visual = true
flash_duration_ms = 100
flash_intensity = 0.3

# Custom keybindings. WARNING: this list REPLACES the defaults entirely.
# If you define this section, reproduce all shortcuts you want to keep.
[keybindings]
bindings = [
    # Tab management
    { key = "t",   mods = ["super"],          action = "new_tab"   },
    { key = "w",   mods = ["super"],          action = "close_tab" },
    { key = "[",   mods = ["super", "shift"], action = "prev_tab"  },
    { key = "]",   mods = ["super", "shift"], action = "next_tab"  },

    # Tab selection
    { key = "1", mods = ["super"], action = "select_tab1" },
    { key = "2", mods = ["super"], action = "select_tab2" },
    { key = "3", mods = ["super"], action = "select_tab3" },
    { key = "4", mods = ["super"], action = "select_tab4" },
    { key = "5", mods = ["super"], action = "select_tab5" },
    { key = "6", mods = ["super"], action = "select_tab6" },
    { key = "7", mods = ["super"], action = "select_tab7" },
    { key = "8", mods = ["super"], action = "select_tab8" },
    { key = "9", mods = ["super"], action = "select_tab9" },

    # Font size
    { key = "equal", mods = ["super"], action = "increase_font_size" },
    { key = "minus", mods = ["super"], action = "decrease_font_size" },
    { key = "0",     mods = ["super"], action = "reset_font_size"    },

    # Clipboard
    { key = "c", mods = ["super"], action = "copy"  },
    { key = "v", mods = ["super"], action = "paste" },

    # Application
    { key = "q", mods = ["super"], action = "quit" },
]
```

---

## Config Directory Layout

```
~/.config/crt/
├── config.toml          # Main configuration file
├── themes/              # Theme CSS files (built-in themes live here)
│   ├── synthwave.css
│   ├── dracula.css
│   └── my-theme.css     # Custom themes go here
├── fonts/               # Optional: custom font files (searched before system fonts)
│   └── MyFont.ttf
└── profile-*.log        # Profiling logs (written when profiling is active)
```
