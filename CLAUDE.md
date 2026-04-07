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
