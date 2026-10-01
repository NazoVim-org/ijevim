# Keybinding Specification

This document tracks keybinding behavior implemented in the current codebase.

## Vim keymap

- Selected via `ijevim vim [file]` (default).
- Full Vim-inspired modal editing: Normal, Insert, Visual, Command, Replace modes.
- Key dispatch in `src/keymap/vim.rs` routes Ctrl+letter bindings and delegates mode-specific handling to methods on `Editor` (`handle_normal`, `handle_insert`, `handle_command`, `handle_visual`, `handle_replace`).
- Vim state machine (pending operators, count prefix, text objects) lives in `Editor` fields.

### Vim-supported operations (complete list in `docs/keymap-vim.md`)

- **Movement**: `h` `j` `k` `l` `w` `b` `e` `0` `^` `$` `gg` `G` `%` `f` `F` `t` `T` `;` `,`
- **Insert**: `i` `a` `A` `o` `O` `R` `s` `S` `C` `D`
- **Delete**: `x` `dd` `dw` `diw` `daw` `d(` `d)` `d[` `d]` `d{` `d}` `d<` `d>` `d{quote}`
- **Change**: `c` `cc` `cw` `ciw` `caw` (same text objects as delete)
- **Yank**: `y` `yy` `yw` `ye` `yiw` `yaw`
- **Visual**: `v` (character), `V` (line), `Ctrl-v` (block), with y/d/c/I/A
- **Search**: `/` `?` `*` `n` `N`
- **Registers**: `"a-`"`z` for yank/delete/put, `+` for system clipboard
- **Marks**: `m{a-z}` `` `{a-z} ` `` `'{a-z}`
- **Macros**: `q{a-z}` to record, `@{a-z}` to play
- **Undo/Redo**: `u` / `Ctrl-r`
- **Repeat**: `.`
- **Scroll**: `Ctrl-d` `Ctrl-u` `Ctrl-y` `Ctrl-e` `Ctrl-b` `Ctrl-f` `zz` `zt` `zb`
- **Indent**: `>>` `<<`
- **Join**: `J`
- **File**: `:w` `:q` `:wq` `:x` `:e` `ZZ` `ZQ`
- **Count prefix**: `3j`, `5dd`, `2dw`, etc.
- **Pending**: `g` prefix for `gg`; `z` prefix for `zz` `zt` `zb`; `Z` prefix for `ZZ` `ZQ`

## Emacs keymap

- Selected via `ijevim emacs [file]`.
- Modeless (always in editor mode), no explicit Normal/Insert distinction.
- Key dispatch in `src/keymap/emacs.rs` handles Ctrl, Meta, prefix sequences, and literal keys.

### Emacs-supported operations (complete list in `docs/keymap-emacs.md`)

- **Movement**: `C-f` `C-b` `C-n` `C-p` `C-a` `C-e` `M-f` `M-b` `M-v`
- **Arrow keys**: `Up` `Down` `Left` `Right` `Home` `End` `PageUp` `PageDown`
- **Editing**: character input, `Enter`, `Backspace`, `Delete`, `C-d` `C-h`
- **Kill/Yank**: `C-k` (kill line), `C-w` (kill word or region), `M-w` (copy region), `C-y` (yank), `M-y` (yank-pop cycle)
- **Mark/Region**: `C-space` (set mark), region-aware `C-w` and `M-w`
- **Undo/Redo**: `C-/` `C-_` `C-?` (undo), `C-x u` / `C-x C-/` (redo)
- **Search**: `C-s` (forward search), `C-r` (backward search), both via command mode
- **Transpose**: `C-t` (transpose characters)
- **File**: `C-x C-s` (save), `C-o` (save shortcut), `C-x C-c` (quit)
- **Screen**: `C-l` (clear + recenter), `C-v` (scroll up one)
- **Abort**: `C-g`
- **Prefix chording**: `C-x` waits for next chord

## Notes

- This specification is implementation-based. If code and this document diverge, the code is the source of truth until this file is updated.
- Vim is the default keymap when no subcommand is given.
