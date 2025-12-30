# winsel

A fast, fuzzy window selector for [Kitty](https://sw.kovidgoyal.net/kitty/) terminal. Select windows with `fzf`, preview their content, and move them between tabs with keyboard shortcuts.

## Author's Note

This started as a Rust project I vibe-coded on vacation in Thailand. Then I rewrote it in Go because the Rust version was slow. The Rust code spawned a subprocess for every kitty operation and fetched all window content upfront. The Go version talks directly to kitty's Unix socket and fetches preview content on demand. It's about 600 lines instead of 2000.

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
- `^A` - Select all windows
- `^B` - Move to background tab (creates "BG" tab if needed)
- `^S` - Split each selected window into its own tab
- `^Y` - Yank window content to clipboard
- `^Del` - Close selected window(s)
- `^O` - Jump mode (type a label to jump to that row)
- `^L` - Clear the search query
- `^/` - Toggle preview pane
- `Esc` - Cancel

### Preview

The preview pane shows a sticky header with:
- **ID** - Window ID
- **Cmd** - Running command
- **Dir** - Current directory
- **Run** - How long the window has been open

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
