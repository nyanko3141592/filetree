# ft - File Tree Explorer

[![CI](https://github.com/nyanko3141592/filetree/actions/workflows/ci.yml/badge.svg)](https://github.com/nyanko3141592/filetree/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

A fast, lightweight file explorer TUI with VSCode-like interface and Vim keybindings.

## Features

- **Git status display** - Color-coded file status (modified, untracked, ignored)
- **Vim-style navigation** - `hjkl` keys, `g`/`G` for jump
- **Mouse support** - Click, double-click, scroll
- **File operations** - Copy, cut, paste, delete, rename
- **Multi-select** - Mark multiple files with `Space`
- **Quick search** - Incremental search with `/`
- **File preview** - Quick view file contents and directory info
- **Hidden files toggle** - Show/hide dotfiles with `.`
- **Path copying** - Copy file path to system clipboard
- **File icons** - Beautiful icons with Nerd Fonts
- **Drag & Drop** - Drop files to copy into selected folder
- **External command execution** - Execute commands on selected files with history support

## Installation

### From crates.io

```bash
cargo install filetree
```

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
| `↑` / `↓` or `j` / `k` | Move up / down |
| `→` or `l` | Expand directory |
| `←` or `h` / `Backspace` | Collapse / Go to parent |
| `g` / `G` | Jump to top / bottom |
| `Tab` | Toggle expand/collapse |
| `H` | Collapse all |
| `L` | Expand all |

### External Commands

| Key | Action |
|-----|--------|
| `Enter` | Execute last command (or prompt for command if first time) |
| `:` | Open command input (use `<filepath>` as placeholder for selected file) |
| `Shift-Enter` | Open command input (if terminal supports) |

**In command input mode:**

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate command history |
| `Enter` | Execute command |
| `Esc` | Cancel |

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
| `o` | Preview file (full screen) |
| `P` | Toggle quick preview (files & directory info) |

### View

| Key | Action |
|-----|--------|
| `.` | Toggle hidden files |
| `R` / `F5` | Reload tree |

### Preview Mode

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll down / up |
| `f` / `b` | Page down / up |
| `g` / `G` | Jump to top / bottom |
| `q` / `Esc` | Close preview |

### Other

| Key | Action |
|-----|--------|
| `c` / `C` | Copy path / filename to clipboard |
| `/` | Search |
| `n` | Next match |
| `?` | Help |
| `q` | Quit |

## Mouse

| Action | Effect |
|--------|--------|
| Click | Select |
| Double-click | Expand/collapse |
| Scroll | Navigate |
| Drag & Drop | Copy file to selected folder |

## Git Status Colors

| Color | Status |
|-------|--------|
| Green | New / Untracked |
| Yellow | Modified |
| Red | Deleted |
| Cyan | Renamed |
| Gray | Ignored |

## External Commands

### Usage

- Press `:` to enter a command, then press `Enter` to execute it
- Use `<filepath>` in your commands as a placeholder for the selected file path
- Command history is automatically saved to `~/.config/filetree/history.txt`
- Navigate command history with `↑` / `↓` keys in command input mode

### Examples

```bash
# First, start Neovim with remote server enabled
nvim --listen /tmp/nvimsocket

# In ft, use this command to open files in the remote Neovim instance
nvim --server /tmp/nvimsocket --remote <filepath>
```

### Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `FILETREE_DEFAULT_CMD` | Default command to execute on first `Enter` press | `code <filepath>` |
| `XDG_CONFIG_HOME` | Configuration directory location | `~/.config` (default) |

## Requirements

- Rust 1.70+
- Terminal with UTF-8 support
- [Nerd Font](https://www.nerdfonts.com/) (recommended for icons)

## License

MIT
