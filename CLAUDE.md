# Terminal Emulator - Claude Code Guidelines

## Command Completion

**❌ DO NOT** implement or extend command completion in the terminal emulator.

**Why:** The project prioritizes `rsh` as the primary shell, which has powerful built-in completion support (command completion, parameter hints, file path completion, etc.). Duplicating this functionality in the terminal emulator is unnecessary and creates maintenance overhead.

**What to do instead:** Let `rsh` handle all completion features natively. Users will get full completion support when using the rsh shell.

---

## Shell Priority

The project uses this shell precedence (in `src/pty.rs`):
1. **rsh** (primary choice) - Preferred shell with advanced features
2. **bash** (fallback)
3. **sh** (last resort)

Ensure any shell integration respects this hierarchy.

---

## Font Configuration

The font is automatically detected and selected from a priority list of Nerd Fonts (`src/config.rs`):

**Priority order:**
1. SauceCodePro Nerd Font
2. SauceCodePro Nerd Font Mono
3. Monokoi Nerd Font
4. Monokoi Nerd Font Mono
5. JetBrains Mono Nerd Font
6. JetBrains Mono NF
7. JetBrainsMono Nerd Font
8. FiraCode Nerd Font (fallback)

**How it works:**
- On startup, `detect_available_fonts()` queries system fonts using `fc-list`
- The first available font from the priority list is automatically selected
- Users can override with custom `font_family` in the config file (`~/.config/terminal_emulator/config.toml`)

**Recommendation:** Ensure at least one Nerd Font is installed for proper icon and symbol rendering.
