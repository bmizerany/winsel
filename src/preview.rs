use crate::{display, kitty_client, utils};
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use base64::Engine;
use std::env;

const PREVIEW_FALLBACK_LINES: usize = 200;

/// Generate live preview for a window ID
pub fn preview_live(win_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    if win_id.is_empty() {
        return Ok(());
    }

    let data = kitty_client::kitty_ls(Some(win_id))?;

    let mut cmd = "<shell>".to_string();
    let mut cwd = "<unknown>".to_string();
    let mut start_ns: i128 = 0;

    'outer: for oswin in &data.0 {
        for tab in &oswin.tabs {
            for win in &tab.windows {
                if win.id.to_string() == win_id {
                    let cmdline = display::join_cmd(win.cmdline());

                    cmd = if !cmdline.is_empty() {
                        cmdline
                    } else {
                        "<shell>".to_string()
                    };

                    cwd = if !win.cwd.is_empty() {
                        win.cwd.clone()
                    } else {
                        "<unknown>".to_string()
                    };
                    start_ns = win.created_at as i128;
                    break 'outer;
                }
            }
        }
    }

    let elapsed = utils::compute_elapsed(start_ns);
    print_header(&cmd, &cwd, elapsed);
    tail_text(win_id)?;
    Ok(())
}

/// Generate static preview from base64-encoded data
pub fn preview_static(
    win_id: &str,
    cmd_b64: &str,
    cwd_b64: &str,
    created_at: i128,
) -> Result<(), Box<dyn std::error::Error>> {
    if win_id.is_empty() {
        return Ok(());
    }
    let cmd_decoded = BASE64_STD
        .decode(cmd_b64.as_bytes())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default();

    let cwd_decoded = BASE64_STD
        .decode(cwd_b64.as_bytes())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "<unknown>".to_string());

    // Use decoded cmd if not empty, otherwise show <shell>
    let display_cmd = if !cmd_decoded.is_empty() {
        cmd_decoded
    } else {
        "<shell>".to_string()
    };

    let elapsed = utils::compute_elapsed(created_at);
    print_header(&display_cmd, &cwd_decoded, elapsed);
    tail_text(win_id)?;
    Ok(())
}

/// Print preview header with command, directory, and elapsed time
fn print_header(cmd: &str, cwd: &str, elapsed: Option<i128>) {
    const RESET: &str = "\x1b[0m";
    const BOLD: &str = "\x1b[1m";

    println!("{}Command:   {}{}", BOLD, RESET, cmd);
    println!("{}Directory: {}{}", BOLD, RESET, cwd);
    println!("{}Running:   {}{}", BOLD, RESET, utils::human_time(elapsed));
    println!();
}

/// Display last N lines of window text (where N = FZF_PREVIEW_LINES or 200)
fn tail_text(win_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let out = kitty_client::kitty_first_ok(|mut cmd| {
        cmd.env("KITTY_GET_TEXT_ANSI", "1").args([
            "get-text",
            "--ansi",
            "--match",
            &format!("id:{win_id}"),
            "--extent=all",
        ]);
        cmd.output()
    })
    .ok_or_else(|| std::io::Error::other("get-text failed"))?;

    let body = String::from_utf8_lossy(&out.stdout);
    let max_lines = env::var("FZF_PREVIEW_LINES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(PREVIEW_FALLBACK_LINES);

    // Take last max_lines and print in normal order (oldest to newest)
    // fzf will scroll to bottom with ~0 to show the most recent
    let lines: Vec<&str> = body.lines().rev().take(max_lines).collect();
    for line in lines.into_iter().rev() {
        println!("{line}");
    }
    Ok(())
}

/// Get last command output for a window
pub fn get_last_command_output(win_id: &str) -> Option<String> {
    if win_id.is_empty() {
        return None;
    }

    let out = kitty_client::kitty_first_ok(|mut cmd| {
        cmd.args([
            "get-text",
            "--match",
            &format!("id:{win_id}"),
            "--extent",
            "last_cmd_output",
        ]);
        cmd.output()
    })?;

    if !out.status.success() {
        return None;
    }

    String::from_utf8(out.stdout).ok()
}
