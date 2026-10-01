# ijevim

A minimal Vim-like TUI editor written in Rust.

## Features

- Vim modes: Normal, Insert, Visual, Command, Replace
- Vim operators: d, y, c, p, >, etc.
- Registers (yank/paste)
- Undo/Redo
- Syntax highlighting (syntect)
- Multi-language plugin support (Lua, Lisp, JavaScript, Rust, Nix)

## CLI usage examples

```sh
# Start with Vim keymap
ijevim vim

# Start with Emacs keymap
ijevim emacs

# Open a file (defaults to Vim keymap)
ijevim <file>
```

## Keybind specification

- See [docs/keybindings.md](docs/keybindings.md) for the current keybinding specification.

## Install

```sh
cargo install ijevim
```

## キーマップ互換レベル

- [Vim キーマップ互換レベル](docs/keymap-vim.md)
- [Emacs キーマップ互換レベル](docs/keymap-emacs.md)

## Development

```sh
cargo build
cargo run -- [file]
cargo test
```

## License

MIT
