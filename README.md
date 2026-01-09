# ft - File Tree Explorer

A VSCode-like file explorer TUI written in Rust.

## Installation

### From source

```bash
cargo install --path .
```

This installs the `ft` command to `~/.cargo/bin/`.

### Manual build

```bash
cargo build --release
# Binary is at ./target/release/ft
```

## Usage

```bash
ft              # Open current directory
ft /path/to/dir # Open specific directory
```

## Key Bindings

| Key | Action |
|-----|--------|
| `j` / `k` / `Arrow` | Move up/down |
| `l` / `Enter` | Expand folder / Select file |
| `h` / `Backspace` | Collapse folder / Go to parent |
| `Tab` | Toggle expand/collapse |
| `g` / `G` | Jump to top/bottom |
| `Space` | Mark/unmark for multi-select |
| `c` | **Copy path to clipboard** |
| `C` | **Copy filename to clipboard** |
| `y` | Yank (copy file) |
| `d` | Cut (for move) |
| `p` | Paste |
| `D` / `Delete` | Delete |
| `r` | Rename |
| `a` | New file |
| `A` | New directory |
| `/` | Search |
| `n` | Next search result |
| `R` / `F5` | Refresh |
| `?` | Help |
| `q` | Quit |

## Mouse Operations

| Action | Effect |
|--------|--------|
| Click | Select item |
| Double-click | Expand/collapse folder |
| Scroll up/down | Navigate up/down |

## Features

- File tree navigation with Vim-like keybindings
- **Mouse support** (click, double-click, scroll)
- **Copy file path to clipboard** (`c` key)
- File operations: copy, move, delete, rename, create
- Multi-file selection with Space key
- Cut & paste as drag-and-drop alternative
- File search
- File type icons (requires Nerd Font)

## Requirements

- Terminal with UTF-8 support
- Nerd Font recommended for icons

## License

MIT
