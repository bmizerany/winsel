# winsel

[![Crates.io](https://img.shields.io/crates/v/winsel?style=flat-square)](https://crates.io/crates/winsel)
[![License](https://img.shields.io/github/license/bmizerany/winsel?style=flat-square)](LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/bmizerany/winsel/ci.yml?style=flat-square)](https://github.com/bmizerany/winsel/actions)

A fast, fuzzy window selector for [Kitty](https://sw.kovidgoyal.net/kitty/) terminal. Select windows with `fzf`, preview their content, and move them between tabs with keyboard shortcuts.

## Author's Note

I vibe-coded this entire project without writing a single line of Rust myself. To this day, I still haven't written any Rust code directly. This was a tool I wanted, and I built it in Rust through vibe-coding while on vacation in Thailand—just to see how it would feel. It felt fun. I hope you enjoy using it as much as I've been enjoying it.

## Features

- **Fuzzy search** across window titles, commands, directories, content, and environment variables
- **Live preview** with sticky header showing command, directory, and runtime
- **Multi-select** to move windows into new tabs or splits
- **Content-aware** - search by anything visible in the terminal output
- **Environment search** - find windows by env vars (VIRTUAL_ENV, NODE_ENV, etc.)
- **Fast** - Rust implementation with efficient kitty API usage

## Requirements

- [Kitty](https://sw.kovidgoyal.net/kitty/) with remote control enabled
- [fzf](https://github.com/junegunn/fzf) (>= 0.67.0 recommended)
- Rust toolchain (for building from source)

## Installation

### From source

```bash
cargo install --path . --locked
```

This installs `winsel` to `~/.cargo/bin/winsel`.

### Kitty configuration

Add to your `kitty.conf`:

```conf
# Enable remote control
allow_remote_control yes
listen_on unix:/tmp/kitty

# Bind winsel to cmd+k (or your preferred key)
map cmd+k launch --type=tab --tab-title=WINSEL --cwd=current winsel
```

## Usage

### Basic

- Run `winsel` or press your configured hotkey (e.g., `cmd+k`)
- Type to filter windows
- `↵ Enter` - Focus selected window
- `Esc` - Cancel

### Multi-select

- `Tab` - Select multiple windows
- `↵ Enter` - Move selections to new tab and focus it
- `^T` - Move to new tab (always creates new tab)
- `^Z` - Move to background tab (creates "BG" tab if needed)
- `^H` - Move to active tab, horizontal split
- `^V` - Move to active tab, vertical split
- `^Y` - Yank last command output to clipboard

### Preview

The preview pane shows:
- **Command** - Running command line
- **Directory** - Current working directory
- **Running** - Uptime (e.g., "5m 30s")
- **Content** - Last N lines of scrollback (controlled by `FZF_PREVIEW_LINES`, default 200)

The header stays pinned at the top while you scroll the content.

## Searchable Content

winsel searches across multiple sources (in priority order):

1. **Working directory** - Abbreviated path with `~`
2. **Command line** - Full command with arguments
3. **Environment variables** - All env vars (name and value)
4. **User variables** - Kitty user_vars
5. **Window content** - Up to 2000 characters of visible terminal output

### Examples

```bash
# Find windows in a virtual environment
VIRTUAL_ENV

# Find Node.js development windows
NODE_ENV

# Find windows showing errors
error 404

# Find windows by directory
~/src/project

# Find by command
python server.py
```

## Configuration

### Environment Variables

- `FZF_PREVIEW_LINES` - Number of scrollback lines to show in preview (default: 200)
- `KITTY_LISTEN_ON` - Kitty socket location (fallback to `KITTY_SOCKET`, then `unix:/tmp/kitty`)

### Temporary Files

- `/tmp/winsel_rows.txt` - Cached window data for fzf
- `/tmp/winsel.log` - Debug logging output

## How It Works

1. **Data Collection** - Queries kitty API for all windows across OS windows and tabs
2. **Enrichment** - Captures window content, env vars, and metadata
3. **Display** - Passes data to fzf with two fields:
   - Field 9: Display (visible in list) - `~/path: command`
   - Field 10: Search (invisible) - Directory, env vars, user vars, window content
4. **Preview** - Live preview via `winsel preview-static` with sticky header
5. **Action** - Executes kitty remote control commands to focus/move windows

## Development

### Building

```bash
cargo build --release
```

### Testing

```bash
cargo test
```

### Running

```bash
cargo run
```

## Architecture

- **`main.rs`** - CLI entry point, fzf orchestration, window operations
- **`display.rs`** - Row formatting, searchable content generation
- **`preview.rs`** - Preview generation with sticky header
- **`kitty_api.rs`** - Type-safe JSON parsing for kitty @ ls output
- **`kitty_client.rs`** - Kitty remote control client
- **`utils.rs`** - Path abbreviation, time formatting, logging

## License

MIT
