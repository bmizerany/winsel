# kitty @ ls Output Reference

The `kitty @ ls` command returns a JSON tree describing all OS windows, tabs, and windows in the kitty terminal. This document provides a comprehensive reference for the output structure.

## Command Synopsis

```
kitty @ ls [options]
```

**Options:**
- `--match <MATCH>` - Filter windows by criteria
- `--match-tab <MATCH>` - Filter tabs by criteria
- `--output-format <FORMAT>` - Output format: `json` (default) or `session`
- `--all-env-vars` - Show all environment variables (not just differences)
- `--self` - List only the window the command runs within

## JSON Structure Overview

The output is a JSON array of OS window objects:

```
[
  OSWindow,
  OSWindow,
  ...
]
```

## Field Reference

### OS Window Object

| Path | Type | Description |
|------|------|-------------|
| `/` | `array` | Root array containing all OS windows |
| `/[n]` | `object` | An OS window object |
| `/[n]/id` | `integer` | Unique identifier for the OS window |
| `/[n]/platform_window_id` | `integer \| null` | Platform-specific window identifier (e.g., X11 window ID) |
| `/[n]/is_focused` | `boolean` | Whether this OS window currently has keyboard focus |
| `/[n]/is_active` | `boolean` | Whether this is the active OS window |
| `/[n]/last_focused` | `boolean` | Whether this was the most recently focused OS window |
| `/[n]/tabs` | `array` | Array of tab objects in this OS window |
| `/[n]/active_tab_history` | `array<integer>` | History of active tab IDs, most recent first |
| `/[n]/wm_class` | `string` | Window manager class name (X11/Wayland) |
| `/[n]/wm_name` | `string` | Window manager instance name |
| `/[n]/background_opacity` | `number` | Background opacity value (0.0 to 1.0) |

### Tab Object

| Path | Type | Description |
|------|------|-------------|
| `/[n]/tabs/[t]` | `object` | A tab object |
| `/[n]/tabs/[t]/id` | `integer` | Unique identifier for the tab |
| `/[n]/tabs/[t]/is_focused` | `boolean` | Whether this tab is focused within its OS window |
| `/[n]/tabs/[t]/is_active` | `boolean` | Whether this is the active tab in its OS window |
| `/[n]/tabs/[t]/title` | `string` | Tab title |
| `/[n]/tabs/[t]/layout` | `string` | Current layout name (e.g., "stack", "tall", "splits", "grid", "vertical") |
| `/[n]/tabs/[t]/layout_state` | `object` | Layout-specific state data (varies by layout) |
| `/[n]/tabs/[t]/layout_opts` | `object` | Layout options/configuration |
| `/[n]/tabs/[t]/enabled_layouts` | `array<string>` | List of layout names available for this tab |
| `/[n]/tabs/[t]/windows` | `array` | Array of window objects in this tab |
| `/[n]/tabs/[t]/groups` | `array` | Array of window group objects |
| `/[n]/tabs/[t]/active_window_history` | `array<integer>` | History of active window IDs, most recent first |

### Window Object

| Path | Type | Description |
|------|------|-------------|
| `/[n]/tabs/[t]/windows/[w]` | `object` | A window object |
| `/[n]/tabs/[t]/windows/[w]/id` | `integer` | Unique identifier for the window |
| `/[n]/tabs/[t]/windows/[w]/is_focused` | `boolean` | Whether this window has keyboard focus |
| `/[n]/tabs/[t]/windows/[w]/is_active` | `boolean` | Whether this is the active window in its tab |
| `/[n]/tabs/[t]/windows/[w]/is_self` | `boolean` | Whether this is the window running the `kitty @ ls` command |
| `/[n]/tabs/[t]/windows/[w]/title` | `string` | Window title |
| `/[n]/tabs/[t]/windows/[w]/pid` | `integer \| null` | Process ID of the child process, or null if unavailable |
| `/[n]/tabs/[t]/windows/[w]/cwd` | `string` | Current working directory of the window |
| `/[n]/tabs/[t]/windows/[w]/cmdline` | `array<string>` | Command line arguments of the shell/process |
| `/[n]/tabs/[t]/windows/[w]/last_reported_cmdline` | `string` | Last command line reported by shell integration |
| `/[n]/tabs/[t]/windows/[w]/last_cmd_exit_status` | `integer` | Exit status of the last executed command |
| `/[n]/tabs/[t]/windows/[w]/env` | `object` | Environment variables (see note below) |
| `/[n]/tabs/[t]/windows/[w]/foreground_processes` | `array` | Array of foreground process descriptors |
| `/[n]/tabs/[t]/windows/[w]/at_prompt` | `boolean` | Whether the shell is at a command prompt (requires shell integration) |
| `/[n]/tabs/[t]/windows/[w]/lines` | `integer` | Number of rows in the window |
| `/[n]/tabs/[t]/windows/[w]/columns` | `integer` | Number of columns in the window |
| `/[n]/tabs/[t]/windows/[w]/user_vars` | `object` | User-defined variables set via escape sequences |
| `/[n]/tabs/[t]/windows/[w]/created_at` | `integer` | Unix timestamp (nanoseconds) when window was created |
| `/[n]/tabs/[t]/windows/[w]/in_alternate_screen` | `boolean` | Whether the window is displaying the alternate screen buffer |
| `/[n]/tabs/[t]/windows/[w]/neighbors` | `object` | Map of neighboring windows by direction |

### Foreground Process Descriptor

| Path | Type | Description |
|------|------|-------------|
| `/[n]/tabs/[t]/windows/[w]/foreground_processes/[p]` | `object` | A process descriptor |
| `/[n]/tabs/[t]/windows/[w]/foreground_processes/[p]/pid` | `integer` | Process ID |
| `/[n]/tabs/[t]/windows/[w]/foreground_processes/[p]/cwd` | `string \| null` | Current working directory of the process |
| `/[n]/tabs/[t]/windows/[w]/foreground_processes/[p]/cmdline` | `array<string> \| null` | Command line arguments |

#### Understanding Foreground Processes

The `foreground_processes` array contains all processes currently in the
**foreground process group** of the window's PTY (pseudo-terminal). This is a
Unix job control concept that determines which processes receive keyboard input
and terminal signals.

**How kitty determines foreground processes:**

1. Each kitty window owns a PTY with an associated file descriptor
2. Kitty calls `tcgetpgrp(fd)` on the PTY file descriptor to get the foreground process group ID (PGID)
3. All processes belonging to that PGID are enumerated and returned

**Relationship to Unix process model:**

```
Terminal Session
├── Session Leader (shell, e.g., zsh, pid=1000, pgid=1000, sid=1000)
│
├── Foreground Process Group (pgid=1001) ← tcgetpgrp() returns this
│   ├── nvim (pid=1001, pgid=1001)
│   └── nvim subprocess (pid=1002, pgid=1001)
│
└── Background Process Group (pgid=1003)
    └── sleep 100 & (pid=1003, pgid=1003)
```

**Key concepts:**

| Term | Description |
|------|-------------|
| **PID** | Process ID - unique identifier for each process |
| **PGID** | Process Group ID - groups related processes (e.g., a pipeline `cat \| grep \| sort` shares one PGID) |
| **SID** | Session ID - groups all processes in a terminal session |
| **Foreground PGID** | The process group that owns the terminal (receives SIGINT, SIGTSTP, etc.) |

**What gets reported:**

- When the shell is idle at a prompt: `foreground_processes` contains only the shell itself
- When running `vim`: `foreground_processes` contains vim and any subprocesses it spawned
- When running `cat | grep | wc`: all three processes appear (same PGID)
- Background jobs (`cmd &`) are **not** included - they have different PGIDs

**Multiple foreground processes:**

When a pipeline or process tree is running, multiple processes share the same PGID and all appear in the array:

```json
{
  "foreground_processes": [
    {"pid": 2001, "cmdline": ["cat", "largefile.txt"]},
    {"pid": 2002, "cmdline": ["grep", "pattern"]},
    {"pid": 2003, "cmdline": ["wc", "-l"]}
  ]
}
```

**Order is not guaranteed.** The array order depends on how the OS enumerates processes:
- On Linux/FreeBSD: order of directory entries when iterating `/proc`
- On macOS: order returned by `sysctl` APIs

The order does **not** reflect:
- Pipeline order (left-to-right in the command)
- Process creation time
- Parent-child relationships

If you need to understand the pipeline structure, you must inspect each process's parent PID (PPID) via `/proc/{pid}/stat` or similar—this information is not included in the `kitty @ ls` output.

**Platform-specific implementation:**

| Platform | Method |
|----------|--------|
| Linux | Parses `/proc/{pid}/stat` to build PGID→PID mappings |
| macOS | Uses native `sysctl` APIs via `process_group_map()` |
| FreeBSD | Similar to Linux, parses `/proc` filesystem |

**Difference from `pid` field:**

The window's `pid` field is the **direct child** process that kitty spawned (typically your shell). The `foreground_processes` array shows what's **currently running in the foreground**, which may be different:

```json
{
  "pid": 1000,           // Shell process (kitty's direct child)
  "cmdline": ["/bin/zsh"],
  "foreground_processes": [
    {
      "pid": 1050,       // Currently running program
      "cmdline": ["nvim", "file.txt"]
    }
  ]
}
```

**Signals and terminal ownership:**

The foreground process group receives terminal-generated signals:
- `SIGINT` (Ctrl+C) - Interrupt
- `SIGQUIT` (Ctrl+\) - Quit with core dump
- `SIGTSTP` (Ctrl+Z) - Suspend

Background process groups that attempt to read from the terminal receive `SIGTTIN` and are stopped. This is why `foreground_processes` accurately reflects what the user is interacting with.

### Neighbors Map

| Path | Type | Description |
|------|------|-------------|
| `/[n]/tabs/[t]/windows/[w]/neighbors` | `object` | Neighboring windows map |
| `/[n]/tabs/[t]/windows/[w]/neighbors/left` | `array<integer>` | Window IDs to the left (optional) |
| `/[n]/tabs/[t]/windows/[w]/neighbors/right` | `array<integer>` | Window IDs to the right (optional) |
| `/[n]/tabs/[t]/windows/[w]/neighbors/top` | `array<integer>` | Window IDs above (optional) |
| `/[n]/tabs/[t]/windows/[w]/neighbors/bottom` | `array<integer>` | Window IDs below (optional) |

### Window Group Object

| Path | Type | Description |
|------|------|-------------|
| `/[n]/tabs/[t]/groups/[g]` | `object` | A window group |
| `/[n]/tabs/[t]/groups/[g]/id` | `integer` | Unique identifier for the group |
| `/[n]/tabs/[t]/groups/[g]/windows` | `array<integer>` | List of window IDs in this group |

### Layout State Object

The `layout_state` field contains layout-specific data. The base structure includes:

| Path | Type | Description |
|------|------|-------------|
| `/[n]/tabs/[t]/layout_state/opts` | `object` | Serialized layout options |
| `/[n]/tabs/[t]/layout_state/class` | `string` | Layout class name |
| `/[n]/tabs/[t]/layout_state/all_windows` | `object` | Window list layout state |
| `/[n]/tabs/[t]/layout_state/all_windows/active_group_idx` | `integer` | Index of the active window group |
| `/[n]/tabs/[t]/layout_state/all_windows/active_group_history` | `array<integer>` | History of active group indices |
| `/[n]/tabs/[t]/layout_state/all_windows/window_groups` | `array` | Array of window group layout states |
| `/[n]/tabs/[t]/layout_state/all_windows/window_groups/[g]/id` | `integer` | Group ID |
| `/[n]/tabs/[t]/layout_state/all_windows/window_groups/[g]/window_ids` | `array<integer>` | Ordered list of window IDs |

## Notes

### Environment Variables

By default, the `env` field only shows environment variables that differ between windows. This optimization reduces output size when many windows share the same environment. Use `--all-env-vars` to include all environment variables.

### Shell Integration

Some fields require kitty shell integration to be enabled:
- `at_prompt` - Requires shell integration to detect prompt state
- `last_reported_cmdline` - Requires shell integration to report commands
- `last_cmd_exit_status` - Requires shell integration to report exit status

### User Variables

The `user_vars` field contains variables set by applications using the OSC 1337 escape sequence:
```
\x1b]1337;SetUserVar=name=base64_value\x07
```

### Matching Windows

The `--match` option supports various criteria:
- `id:<id>` - Match by window ID
- `title:<regex>` - Match by window title
- `pid:<pid>` - Match by process ID
- `cwd:<path>` - Match by current working directory
- `cmdline:<regex>` - Match by command line
- `num:<n>` - Match by window number (0-indexed within tab)
- `env:<VAR>=<regex>` - Match by environment variable
- `var:<name>=<regex>` - Match by user variable
- `state:<state>` - Match by state (focused, active, needs_attention, parent_focused, parent_active, self, overlay_parent)
- `neighbor:<direction>` - Match neighbor in direction (left, right, top, bottom)
- `recent:<n>` - Match by recency (0 = most recent)

Multiple criteria can be combined with `and`, `or`, and `not`.

## Example Output

```json
[
  {
    "id": 1,
    "platform_window_id": 12345678,
    "is_focused": true,
    "is_active": true,
    "last_focused": true,
    "wm_class": "kitty",
    "wm_name": "kitty",
    "background_opacity": 1.0,
    "active_tab_history": [1],
    "tabs": [
      {
        "id": 1,
        "is_focused": true,
        "is_active": true,
        "title": "~/projects",
        "layout": "splits",
        "layout_state": {},
        "layout_opts": {},
        "enabled_layouts": ["splits", "stack", "tall", "fat", "grid"],
        "active_window_history": [1, 2],
        "groups": [
          {"id": 1, "windows": [1]},
          {"id": 2, "windows": [2]}
        ],
        "windows": [
          {
            "id": 1,
            "is_focused": true,
            "is_active": true,
            "is_self": true,
            "title": "nvim",
            "pid": 12345,
            "cwd": "/home/user/projects",
            "cmdline": ["/usr/bin/zsh"],
            "last_reported_cmdline": "nvim .",
            "last_cmd_exit_status": 0,
            "env": {"EDITOR": "nvim"},
            "foreground_processes": [
              {
                "pid": 12346,
                "cwd": "/home/user/projects",
                "cmdline": ["nvim", "."]
              }
            ],
            "at_prompt": false,
            "lines": 40,
            "columns": 120,
            "user_vars": {},
            "created_at": 1704067200000000000,
            "in_alternate_screen": true,
            "neighbors": {
              "right": [2]
            }
          },
          {
            "id": 2,
            "is_focused": false,
            "is_active": false,
            "is_self": false,
            "title": "zsh",
            "pid": 12350,
            "cwd": "/home/user",
            "cmdline": ["/usr/bin/zsh"],
            "last_reported_cmdline": "",
            "last_cmd_exit_status": 0,
            "env": {},
            "foreground_processes": [
              {
                "pid": 12350,
                "cwd": "/home/user",
                "cmdline": ["/usr/bin/zsh"]
              }
            ],
            "at_prompt": true,
            "lines": 40,
            "columns": 60,
            "user_vars": {},
            "created_at": 1704067300000000000,
            "in_alternate_screen": false,
            "neighbors": {
              "left": [1]
            }
          }
        ]
      }
    ]
  }
]
```

## References

- [kitty Remote Control Documentation](https://sw.kovidgoyal.net/kitty/remote-control/)
- [kitty Source Code](https://github.com/kovidgoyal/kitty)
  - `kitty/rc/ls.py` - ls command implementation
  - `kitty/boss.py` - `list_os_windows()` method
  - `kitty/tabs.py` - `list_tabs()` method
  - `kitty/window.py` - `as_dict()` method
  - `kitty/window_list.py` - Window group serialization
  - `kitty/child.py` - ProcessDesc type definition and foreground process detection

### Unix Process Model References

- [tcgetpgrp(3) - Linux manual page](https://man7.org/linux/man-pages/man3/tcgetpgrp.3.html)
- [Process Groups, Jobs and Sessions](https://biriukov.dev/docs/fd-pipe-session-terminal/3-process-groups-jobs-and-sessions/)
- [APUE Chapter 9 - Process Relationships](https://notes.shichao.io/apue/ch9/)
- [GNU C Library - Job Control](https://ftp.gnu.org/old-gnu/Manuals/glibc-2.2.3/html_chapter/libc_27.html)
