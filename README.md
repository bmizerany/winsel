# winsel

A fast, fuzzy window selector for [Kitty](https://sw.kovidgoyal.net/kitty/) terminal. Select windows with `fzf`, preview their content, and move them between tabs with keyboard shortcuts.

## Features

- **Fuzzy search** across window titles and directories
- **Live preview** with scrollback content and metadata
- **Multi-select** to group windows into tabs or close in bulk
- **Fast** - direct socket communication, lazy content loading

## Requirements

- [Kitty](https://sw.kovidgoyal.net/kitty/) with remote control enabled
- [fzf](https://github.com/junegunn/fzf)
- Go 1.23+ (for building)

## Installation

```bash
go install blake.io/winsel@latest
```

Or from source:

```bash
go build -o ~/go/bin/winsel .
```

## Kitty configuration

Add to your `kitty.conf`:

```conf
allow_remote_control yes
listen_on unix:/tmp/kitty

map cmd+k launch --type=overlay --title=WINSEL winsel
```

## Usage

### Keys

- `Enter` - Focus selected window(s); if multiple, group into new tab
- `^a` - Select all windows
- `^b` - Move to background tab (creates "BG" tab if needed)
- `^s` - Split each selected window into its own tab
- `^y` - Yank window content to clipboard
- `^c` - Close selected window(s)
- `^o` - Jump mode (type a label to jump to that row)
- `^l` - Clear the search query
- `^/` - Toggle preview pane
- `esc` - Cancel

### Preview

The preview pane shows a sticky header with:
- **ID** - Window identifier (OS_LETTER:TAB_ID:WIN_ID)
- **Dir** - Current directory
- **Cmd** - Running command

Below the header is the terminal scrollback with ANSI colors, scrolled to show the most recent output.

## Architecture

```
main.go        - CLI, fzf orchestration, window operations
kitty/client.go - Direct socket client for kitty remote control
```

The client maintains a connection pool and speaks kitty's escape-sequence
protocol directly, avoiding the overhead of spawning `kitty @` subprocesses.

## License

MIT
