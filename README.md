# ft - File Tree Explorer

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

A fast, lightweight file explorer TUI with VSCode-like interface and Vim keybindings.

![ft demo](https://via.placeholder.com/800x400?text=Demo+Screenshot)

## Features

- **Vim-style navigation** - `hjkl` keys, `g`/`G` for jump
- **Mouse support** - Click, double-click, scroll
- **File operations** - Copy, cut, paste, delete, rename
- **Multi-select** - Mark multiple files with `Space`
- **Quick search** - Incremental search with `/`
- **Path copying** - Copy file path to system clipboard
- **File icons** - Beautiful icons with Nerd Fonts

## Installation

### From source

```bash
git clone https://github.com/nyanko3141592/filetree.git
cd filetree
cargo install --path .
```

### Build manually

```bash
cargo build --release
# Binary: ./target/release/ft
```

## Usage

```bash
ft              # Current directory
ft ~/Documents  # Specific directory
```

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `l` / `Enter` | Expand directory |
| `h` / `Backspace` | Collapse / Go to parent |
| `g` / `G` | Jump to top / bottom |
| `Tab` | Toggle expand/collapse |

### File Operations

| Key | Action |
|-----|--------|
| `Space` | Mark/unmark file |
| `y` | Yank (copy) |
| `d` | Cut |
| `p` | Paste |
| `D` | Delete |
| `r` | Rename |
| `a` / `A` | New file / directory |

### Other

| Key | Action |
|-----|--------|
| `c` / `C` | Copy path / filename to clipboard |
| `/` | Search |
| `n` | Next match |
| `R` | Refresh |
| `?` | Help |
| `q` | Quit |

## Mouse

| Action | Effect |
|--------|--------|
| Click | Select |
| Double-click | Expand/collapse |
| Scroll | Navigate |

## Requirements

- Rust 1.70+
- Terminal with UTF-8 support
- [Nerd Font](https://www.nerdfonts.com/) (recommended for icons)

## License

MIT
