use crate::kitty_api::KittyData;
use crate::utils;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use base64::Engine;
use std::process::Command;

// ANSI color codes for highlighting
pub const COLOR_RESET: &str = "\x1b[0m";
pub const COLOR_PATH: &str = "\x1b[35m"; // Magenta for paths
pub const COLOR_CMD: &str = "\x1b[32m"; // Green for commands
pub const COLOR_META: &str = "\x1b[90m"; // Dim/gray for metadata

/// Build FZF rows from kitty data
pub fn build_rows(data: &KittyData, self_id: Option<&str>) -> Vec<String> {
    let mut rows = Vec::new();

    for oswin in &data.0 {
        let os_rank = if oswin.is_focused || oswin.last_focused {
            0
        } else {
            1
        };

        for tab in &oswin.tabs {
            let tab_rank = if tab.is_active || tab.is_focused { 0 } else { 1 };
            let history = &tab.active_window_history;

            for win in &tab.windows {
                let wid = win.id.to_string();

                // Filter out self window
                if self_id.is_some() && self_id == Some(wid.as_str()) {
                    continue;
                }

                let wtitle = utils::clean(&win.title);
                let cmdline = join_cmd(win.cmdline());

                // Filter out WINSEL windows
                if wtitle.to_uppercase() == "WINSEL" || cmdline.to_lowercase().contains("winsel") {
                    continue;
                }

                let cwd = utils::clean(&win.cwd);
                let encoded_cmd = BASE64_STD.encode(cmdline.as_bytes());
                let encoded_cwd = BASE64_STD.encode(cwd.as_bytes());

                let title_col = if !wtitle.is_empty() {
                    wtitle.clone()
                } else {
                    cmdline.clone()
                };

                // Build smart label for display
                let smart_label = build_smart_label(&wtitle, &cmdline, &cwd);

                // Get window content for searching
                let window_content = get_window_text(&wid).unwrap_or_default();

                // Build extra searchable content (will be in separate field)
                let extra_search = build_extra_searchable_content(&cwd, &window_content, &win.env, &win.user_vars);

                // Calculate rank
                let hist_idx = history
                    .iter()
                    .position(|&v| v == win.id)
                    .unwrap_or(history.len() + 1) as i64;

                let rank = (
                    if win.is_focused { 0 } else { 1 },
                    os_rank,
                    tab_rank,
                    hist_idx,
                    -win.created_at,
                );

                // Field 9: display only, Field 10: searchable only
                let row = format!(
                    "{wid}\t{title_col}\t{}\t{cmdline}\t{encoded_cmd}\t{cwd}\t{}\t{encoded_cwd}\t{smart_label}\t{extra_search}",
                    cmdline, // disp_title
                    win.created_at
                );

                rows.push((rank, row));
            }
        }
    }

    rows.sort_by_key(|item| item.0);
    rows.into_iter().map(|(_, line)| line).collect()
}

/// Join command array into a string, basename the first arg if it's an absolute path
pub fn join_cmd(arr: &[String]) -> String {
    arr.iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let cleaned = utils::clean(s);
            if cleaned.is_empty() {
                return None;
            }

            // Only basename the first argument (the executable) if it's an absolute path
            Some(if i == 0 && cleaned.starts_with('/') {
                std::path::Path::new(&cleaned)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&cleaned)
                    .to_string()
            } else {
                cleaned
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build smart label with colored path and command
pub fn build_smart_label(_window_title: &str, cmdline: &str, cwd: &str) -> String {
    // Abbreviate path
    let mut path = if !cwd.is_empty() {
        utils::abbreviate_path(cwd)
    } else {
        "~".to_string()
    };

    // Add trailing slash if path is just ~
    if path == "~" {
        path.push('/');
    }

    // Determine what to display - use cmdline or fallback to <shell>
    let display = if !cmdline.is_empty() {
        cmdline.to_string()
    } else {
        "<shell>".to_string()
    };

    // Format with ANSI colors: magenta path, green command
    format!(
        "{}{}{}: {}{}{}",
        COLOR_PATH, path, COLOR_RESET, COLOR_CMD, display, COLOR_RESET
    )
}

/// Build extra searchable content (metadata + window content + env vars)
pub fn build_extra_searchable_content(
    cwd: &str,
    window_content: &str,
    env: &std::collections::HashMap<String, String>,
    user_vars: &std::collections::HashMap<String, String>,
) -> String {
    let mut parts = vec![format!("{}Directory: {}{}", COLOR_META, cwd, COLOR_RESET)];

    // Add all env vars (both name and value are searchable)
    for (key, value) in env {
        parts.push(format!("{}{}={}{}", COLOR_META, key, value, COLOR_RESET));
    }

    // Add all user vars
    for (key, value) in user_vars {
        parts.push(format!("{}{}={}{}", COLOR_META, key, value, COLOR_RESET));
    }

    // Add window content if present
    if !window_content.is_empty() {
        parts.push(window_content.to_string());
    }

    parts.join(" ")
}

/// Get window text content via kitty API (used for searching)
pub fn get_window_text(win_id: &str) -> Option<String> {
    if win_id.is_empty() {
        return None;
    }

    // Try multiple kitty sockets
    let candidates = [
        std::env::var("KITTY_LISTEN_ON").ok(),
        std::env::var("KITTY_SOCKET").ok(),
        Some("unix:/tmp/kitty".to_string()),
        Some("unix:@kitty".to_string()),
    ];

    for socket in candidates.into_iter().flatten() {
        if socket.is_empty() {
            continue;
        }

        let mut cmd = Command::new("kitty");
        cmd.args(["@", "--to", &socket, "get-text", "--match", &format!("id:{win_id}"), "--extent=all"]);

        if let Ok(out) = cmd.output() {
            if out.status.success() {
                if let Ok(text) = String::from_utf8(out.stdout) {
                    // Clean up the text for searching - remove excessive whitespace
                    let cleaned = text
                        .lines()
                        .map(|line| line.trim())
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");

                    // Limit length to avoid slowing down fzf (keep ~2000 chars)
                    const MAX_LEN: usize = 2000;
                    if cleaned.len() > MAX_LEN {
                        return Some(cleaned[..MAX_LEN].to_string());
                    } else {
                        return Some(cleaned);
                    }
                }
            }
        }
    }

    // Final fallback: no explicit target
    let mut cmd = Command::new("kitty");
    cmd.args(["@", "get-text", "--match", &format!("id:{win_id}"), "--extent=all"]);

    if let Ok(out) = cmd.output() {
        if out.status.success() {
            if let Ok(text) = String::from_utf8(out.stdout) {
                let cleaned = text
                    .lines()
                    .map(|line| line.trim())
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");

                const MAX_LEN: usize = 2000;
                if cleaned.len() > MAX_LEN {
                    return Some(cleaned[..MAX_LEN].to_string());
                } else {
                    return Some(cleaned);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kitty_api::{OsWindow, Tab, Window};

    // join_cmd tests
    #[test]
    fn test_join_cmd_empty() {
        assert_eq!(join_cmd(&[]), "");
    }

    #[test]
    fn test_join_cmd_single() {
        assert_eq!(join_cmd(&["vim".to_string()]), "vim");
    }

    #[test]
    fn test_join_cmd_multiple() {
        assert_eq!(
            join_cmd(&["vim".to_string(), "file.txt".to_string()]),
            "vim file.txt"
        );
    }

    #[test]
    fn test_join_cmd_absolute_path() {
        assert_eq!(
            join_cmd(&["/usr/bin/vim".to_string(), "file.txt".to_string()]),
            "vim file.txt"
        );
    }

    #[test]
    fn test_join_cmd_filters_empty() {
        assert_eq!(
            join_cmd(&["vim".to_string(), "".to_string(), "file.txt".to_string()]),
            "vim file.txt"
        );
    }

    // build_smart_label tests (keep existing)
    #[test]
    fn test_smart_label_no_title() {
        let label = build_smart_label("", "go run -C ~/src/ollama . serve", "/Users/bmizerany/.config/kitty");
        let expected = format!(
            "{}{}{}: {}{}{}",
            COLOR_PATH,
            "~/.config/kitty",
            COLOR_RESET,
            COLOR_CMD,
            "go run -C ~/src/ollama . serve",
            COLOR_RESET
        );
        assert_eq!(label, expected);
    }

    #[test]
    fn test_smart_label_exact_match() {
        let cmd = "go run -C ~/src/ollama . serve";
        let label = build_smart_label(cmd, cmd, "/Users/bmizerany/.config/kitty");
        let expected = format!(
            "{}{}{}: {}{}{}",
            COLOR_PATH,
            "~/.config/kitty",
            COLOR_RESET,
            COLOR_CMD,
            cmd,
            COLOR_RESET
        );
        assert_eq!(label, expected);
    }

    #[test]
    fn test_smart_label_different_title() {
        let title = "Ollama Server";
        let cmd = "go run -C ~/src/ollama . serve";
        let label = build_smart_label(title, cmd, "/Users/bmizerany/.config/kitty");
        let expected = format!(
            "{}{}{}: {}{}{}",
            COLOR_PATH,
            "~/.config/kitty",
            COLOR_RESET,
            COLOR_CMD,
            cmd,
            COLOR_RESET
        );
        assert_eq!(label, expected);
    }

    #[test]
    fn test_smart_label_no_cmdline_fallback_to_shell() {
        let title = "Window Title";
        let label = build_smart_label(title, "", "/Users/bmizerany/.config/kitty");
        let expected = format!(
            "{}{}{}: {}{}{}",
            COLOR_PATH,
            "~/.config/kitty",
            COLOR_RESET,
            COLOR_CMD,
            "<shell>",
            COLOR_RESET
        );
        assert_eq!(label, expected);
    }

    #[test]
    fn test_smart_label_fallback_to_shell() {
        let label = build_smart_label("", "", "/Users/bmizerany/.config/kitty");
        let expected = format!(
            "{}{}{}: {}{}{}",
            COLOR_PATH,
            "~/.config/kitty",
            COLOR_RESET,
            COLOR_CMD,
            "<shell>",
            COLOR_RESET
        );
        assert_eq!(label, expected);
    }

    // build_rows tests with typed data
    #[test]
    fn test_build_rows_empty() {
        let data = KittyData(vec![]);
        let rows = build_rows(&data, None);
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn test_build_rows_filters_self() {
        let data = KittyData(vec![OsWindow {
            id: 1,
            is_focused: true,
            last_focused: false,
            tabs: vec![Tab {
                id: 1,
                is_active: true,
                is_focused: true,
                title: "Main".to_string(),
                active_window_history: vec![],
                windows: vec![Window {
                    id: 123,
                    is_focused: true,
                    title: "vim".to_string(),
                    cwd: "/home/user".to_string(),
                    created_at: 1700000000,
                    last_reported_cmdline: vec!["vim".to_string()],
                    foreground_processes: vec![],
                }],
            }],
        }]);

        let rows = build_rows(&data, Some("123"));
        assert_eq!(rows.len(), 0); // Self window should be filtered
    }

    #[test]
    fn test_build_rows_filters_winsel() {
        let data = KittyData(vec![OsWindow {
            id: 1,
            is_focused: true,
            last_focused: false,
            tabs: vec![Tab {
                id: 1,
                is_active: true,
                is_focused: true,
                title: "Main".to_string(),
                active_window_history: vec![],
                windows: vec![
                    Window {
                        id: 1,
                        is_focused: false,
                        title: "WINSEL".to_string(),
                        cwd: "/home/user".to_string(),
                        created_at: 1700000000,
                        last_reported_cmdline: vec![],
                        foreground_processes: vec![],
                    },
                    Window {
                        id: 2,
                        is_focused: false,
                        title: "bash".to_string(),
                        cwd: "/home/user".to_string(),
                        created_at: 1700000000,
                        last_reported_cmdline: vec!["winsel".to_string()],
                        foreground_processes: vec![],
                    },
                ],
            }],
        }]);

        let rows = build_rows(&data, None);
        assert_eq!(rows.len(), 0); // Both WINSEL windows should be filtered
    }

    #[test]
    fn test_build_rows_single_window() {
        let data = KittyData(vec![OsWindow {
            id: 1,
            is_focused: true,
            last_focused: false,
            tabs: vec![Tab {
                id: 1,
                is_active: true,
                is_focused: true,
                title: "Main".to_string(),
                active_window_history: vec![1],
                windows: vec![Window {
                    id: 1,
                    is_focused: true,
                    title: "vim".to_string(),
                    cwd: "/home/user".to_string(),
                    created_at: 1700000000,
                    last_reported_cmdline: vec!["vim".to_string(), "file.txt".to_string()],
                    foreground_processes: vec![],
                }],
            }],
        }]);

        let rows = build_rows(&data, None);
        assert_eq!(rows.len(), 1);
        // Row should contain window ID
        assert!(rows[0].starts_with("1\t"));
    }
}
