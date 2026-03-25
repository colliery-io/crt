# Keyboard Shortcuts Reference

Two categories of shortcuts exist in CRT:

- **Configurable** — defined in `[[keybindings.bindings]]` in `config.toml`. Providing a `bindings` array replaces all defaults; there is no merge.
- **Hardcoded** — built into the application and cannot be changed via config.

On macOS, `Cmd` refers to the Command key (`⌘`). On Linux, the `super` modifier maps to the Windows key; `Cmd` shortcuts listed here use `Ctrl` instead on Linux where noted.

---

## Configurable Shortcuts (Default Bindings)

These are the defaults. They can be replaced by defining `[[keybindings.bindings]]` in `config.toml`.

### Tab Management

| Shortcut | Action |
|---|---|
| `Cmd+T` | Open new tab |
| `Cmd+W` | Close active tab (closes window if only one tab remains) |
| `Cmd+Shift+[` | Switch to previous tab |
| `Cmd+Shift+]` | Switch to next tab |
| `Cmd+1` | Switch to tab 1 |
| `Cmd+2` | Switch to tab 2 |
| `Cmd+3` | Switch to tab 3 |
| `Cmd+4` | Switch to tab 4 |
| `Cmd+5` | Switch to tab 5 |
| `Cmd+6` | Switch to tab 6 |
| `Cmd+7` | Switch to tab 7 |
| `Cmd+8` | Switch to tab 8 |
| `Cmd+9` | Switch to tab 9 |

### Font Size

| Shortcut | Action |
|---|---|
| `Cmd+=` | Increase font size |
| `Cmd+-` | Decrease font size |
| `Cmd+0` | Reset font size to configured default |

### Clipboard

| Shortcut | Action |
|---|---|
| `Cmd+C` | Copy selected text to clipboard |
| `Cmd+V` | Paste from clipboard |

### Application

| Shortcut | Action |
|---|---|
| `Cmd+Q` | Quit CRT |

---

## Hardcoded Shortcuts

These shortcuts are always active and cannot be changed in `config.toml`.

### Window Management

| Shortcut | Action |
|---|---|
| `Cmd+N` | Open new window |
| `Cmd+Shift+W` | Close current window |
| `Cmd+Shift+R` | Rename current window (opens inline dialog) |
| `Ctrl+Cmd+F` | Toggle fullscreen |
| `Cmd+M` | Minimize window |

### Editing

| Shortcut | Action |
|---|---|
| `Cmd+A` | Select all terminal content |
| `Cmd+K` | Clear scrollback buffer |

### Search

| Shortcut | Action |
|---|---|
| `Cmd+F` | Open search bar |
| `Enter` (search active) | Find next match |
| `Escape` | Close search bar / dismiss dialog |

### Scrollback Navigation

| Shortcut | Action |
|---|---|
| `Shift+PageUp` | Scroll up one page |
| `Shift+PageDown` | Scroll down one page |
| `Shift+Home` | Jump to top of scrollback |
| `Shift+End` | Jump to bottom of scrollback |
| `Cmd+Shift+ArrowLeft` (macOS) | Jump to top of scrollback |
| `Cmd+Shift+ArrowRight` (macOS) | Jump to bottom of scrollback |

### Diagnostics

| Shortcut | Action |
|---|---|
| `Cmd+Option+P` | Toggle runtime profiling on/off |

### Mouse Shortcuts

| Action | Behavior |
|---|---|
| `Cmd+Click` on URL | Open URL in default browser |
| Double-click tab title | Begin inline tab rename |
| Right-click terminal | Open context menu (includes theme switching) |

---

## Context Menu Navigation (Keyboard)

When the right-click context menu is open:

| Key | Action |
|---|---|
| `Arrow Down` | Focus next menu item |
| `Arrow Up` | Focus previous menu item |
| `Enter` | Activate focused item |
| `Escape` | Dismiss menu |

---

## Dialog Input (Rename Window / Rename Tab)

When a rename dialog is active:

| Key | Action |
|---|---|
| Any printable character | Append to name input |
| `Backspace` | Delete last character |
| `Enter` | Confirm rename (empty input resets to default title) |
| `Escape` | Cancel rename |

---

## Modifier Key Names

| Config value | macOS key | Linux/Windows key |
|---|---|---|
| `"super"` | `Cmd` (⌘) | Windows key |
| `"shift"` | `Shift` | `Shift` |
| `"ctrl"` | `Control` | `Control` |
| `"alt"` | `Option` (⌥) | `Alt` |
