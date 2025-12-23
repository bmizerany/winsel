mod display;
mod error;
mod kitty_api;
mod kitty_client;
mod preview;
mod utils;

use std::collections::HashSet;
use std::env;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("preview") => {
            let win_id = args.next().unwrap_or_default();
            preview::preview_live(&win_id)?;
        }
        Some("apply") => {
            let ids: Vec<String> = args.collect();
            apply_selection(&ids)?;
        }
        Some("apply-bg") => {
            let ids: Vec<String> = args.collect();
            apply_selection_background(&ids)?;
        }
        Some("apply-tab") => {
            let ids: Vec<String> = args.collect();
            move_selection_new_tab(&ids)?;
        }
        Some("split-h") => {
            let ids: Vec<String> = args.collect();
            move_selection_split(&ids, "hsplit")?;
        }
        Some("split-v") => {
            let ids: Vec<String> = args.collect();
            move_selection_split(&ids, "vsplit")?;
        }
        Some("yank") => {
            let ids: Vec<String> = args.collect();
            yank_last_command(&ids)?;
        }
        Some("preview-static") => {
            let win_id = args.next().unwrap_or_default();
            let cmd_b64 = args.next().unwrap_or_default();
            let cwd = args.next().unwrap_or_default();
            let created = args
                .next()
                .and_then(|v| v.parse::<i128>().ok())
                .unwrap_or_default();
            preview::preview_static(&win_id, &cmd_b64, &cwd, created)?;
        }
        _ => run_main()?,
    }
    Ok(())
}

fn run_main() -> Result<(), Box<dyn std::error::Error>> {
    let self_id = env::var("KITTY_WINDOW_ID").ok();
    let has_tty_marker = env::var("WINSEL_HAS_TTY").is_ok();

    utils::log_msg(format!(
        "start self_id={:?} pid={} cwd={} has_tty_marker={}",
        self_id,
        std::process::id(),
        env::current_dir().unwrap_or_default().display(),
        has_tty_marker
    ));

    // Check if we have a TTY
    let has_tty = unsafe {
        let fd = 0; // stdin
        let ptr = libc::ttyname(fd);
        !ptr.is_null()
    };

    utils::log_msg(format!("has_tty={}", has_tty));

    // Warn if no TTY but continue - fzf can open /dev/tty itself for keyboard input
    if !has_tty {
        utils::log_msg("warning: stdin not a TTY, but continuing");
    }

    let data = kitty_client::kitty_ls(None)?;

    // Capture the tab we should return focus to (the tab active before the WINSEL tab).
    let current_tab_id = self_id
        .as_deref()
        .and_then(|id| find_tab_id_for_window(&data, id));
    let current_tab_title = current_tab_id
        .as_deref()
        .and_then(|tid| find_tab_title(&data, tid));
    let origin_tab_id = if current_tab_title
        .as_deref()
        .map(|t| t.eq_ignore_ascii_case("winsel"))
        .unwrap_or(false)
    {
        find_recent_tab_id(1)
    } else {
        current_tab_id.clone()
    };
    if let Some(ref origin) = origin_tab_id {
        env::set_var("WINSEL_ORIGIN_TAB", origin);
        utils::log_msg(format!("origin_tab_id={origin}"));
    }

    let start = std::time::Instant::now();
    let rows = display::build_rows(&data, self_id.as_deref());
    utils::log_msg(format!("build_rows took {:?}", start.elapsed()));

    utils::log_msg(format!("rows={}", rows.len()));

    if rows.is_empty() {
        kitty_client::kitty_notify("WINSEL: no other windows");
        return Ok(());
    }

    if Command::new("fzf")
        .arg("--version")
        .stdout(Stdio::null())
        .status()
        .is_err()
    {
        kitty_client::kitty_notify("WINSEL: fzf not found");
        return Ok(());
    }

    let exe = env::current_exe()?;
    let preview_cmd = "\"$WINSEL_BIN\" preview-static {1} {5} {8} {7}".to_string();

    utils::log_msg(format!(
        "spawn fzf rows={} env WINSEL_BIN={}",
        rows.len(),
        exe.display()
    ));

    let apply_cmd = "$WINSEL_BIN apply {+1}";
    let apply_bg_cmd = "$WINSEL_BIN apply-bg {+1}";
    let apply_tab_cmd = "$WINSEL_BIN apply-tab {+1}";
    let split_h_cmd = "$WINSEL_BIN split-h {+1}";
    let split_v_cmd = "$WINSEL_BIN split-v {+1}";
    let yank_cmd = "$WINSEL_BIN yank {+1}";

    // Write rows to a temp file so we can keep stdin on the TTY (avoids fzf auto-exiting when stdin is not a tty).
    let tmp_path = "/tmp/winsel_rows.txt";
    std::fs::write(tmp_path, rows.join("\n"))?;

    let mut fzf_cmd = Command::new("fzf");
    fzf_cmd.env("WINSEL_BIN", exe.display().to_string());
    if let Some(ref origin) = origin_tab_id {
        fzf_cmd.env("WINSEL_ORIGIN_TAB", origin);
    }
    fzf_cmd
        .env("FZF_DEFAULT_OPTS", "")
        .env("FZF_DEFAULT_COMMAND", format!("cat {tmp_path}"))
        .arg("--prompt=WINSEL> ")
        .arg("--multi")
        .arg("--ansi")
        .arg("--delimiter")
        .arg("\t")
        .arg("--with-nth=9")
        .arg("--layout=reverse")
        .arg("--no-select-1")
        .arg("--no-exit-0")
        .arg("--info=right")
        .arg("--border")
        .arg("--border-label-pos=bottom")
        .arg("--border-label= ↵:focus z:bg t:tab h:hsplit v:vsplit y:yank ")
        .arg("--preview")
        .arg(preview_cmd)
        .arg("--preview-window=right:60%,nowrap,follow,~4")
        .arg("--bind")
        .arg(format!(
            "enter:execute-silent({apply_cmd})+accept,\
ctrl-z:execute-silent({apply_bg_cmd})+accept,\
ctrl-t:execute-silent({apply_tab_cmd})+accept,\
ctrl-h:execute-silent({split_h_cmd})+accept,\
ctrl-v:execute-silent({split_v_cmd})+accept,\
ctrl-y:execute-silent({yank_cmd})"
        ))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = fzf_cmd.spawn()?;

    utils::log_msg("fzf spawned successfully");

    utils::log_msg("waiting for fzf to exit");
    let status = child.wait()?;
    utils::log_msg(format!("fzf exit status={}", status));

    Ok(())
}

fn unique_ids(ids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut uniq = Vec::new();

    for id in ids {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            uniq.push(trimmed.to_string());
        }
    }

    uniq
}

fn apply_selection(ids: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let uniq = unique_ids(ids);

    if uniq.is_empty() {
        return Ok(());
    }

    if uniq.len() == 1 {
        focus_window(&uniq[0]);
        return Ok(());
    }

    move_selected(&uniq, true, None)
}

fn apply_selection_background(ids: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let uniq = unique_ids(ids);

    if uniq.is_empty() {
        return Ok(());
    }

    let origin_tab = env::var("WINSEL_ORIGIN_TAB").ok().filter(|v| !v.is_empty());
    let bg_tab_id = find_or_create_bg_tab()?;

    for wid in &uniq {
        let target = format!("id:{wid}");
        kitty_client::kitty_first_ok(|mut cmd| {
            cmd.args([
                "detach-window",
                "--match",
                &target,
                "--stay-in-tab",
                "--target-tab",
                &format!("id:{bg_tab_id}"),
            ]);
            cmd.status()
        });
    }

    if let Some(tab_id) = origin_tab {
        focus_tab_by_id(&tab_id);
    } else {
        focus_previous_tab();
    }

    Ok(())
}

fn move_selection_new_tab(ids: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let uniq = unique_ids(ids);

    if uniq.is_empty() {
        return Ok(());
    }

    move_selected(&uniq, true, None)
}

fn move_selection_split(ids: &[String], location: &str) -> Result<(), Box<dyn std::error::Error>> {
    let uniq = unique_ids(ids);

    if uniq.is_empty() {
        return Ok(());
    }

    let tab_id = env::var("WINSEL_ORIGIN_TAB")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| find_recent_tab_id(1))
        .or_else(|| find_recent_tab_id(0));

    let tab_id = match tab_id {
        Some(id) => id,
        None => {
            kitty_client::kitty_notify("WINSEL: no target tab for split move");
            return Ok(());
        }
    };

    let tab_match = format!("id:{tab_id}");

    // Focus the target tab first so splits have a reference window
    focus_tab_by_id(&tab_id);

    for wid in &uniq {
        let target = format!("id:{wid}");
        let result = kitty_client::kitty_first_ok(|mut cmd| {
            cmd.args([
                "detach-window",
                "--match",
                &target,
                "--target-tab",
                &tab_match,
                "--location",
                location,
            ]);
            cmd.status()
        });

        if result.is_none() {
            utils::log_msg(format!("split move failed for window {wid}"));
        }
    }

    // Focus the first moved window
    focus_window(&uniq[0]);

    Ok(())
}

fn yank_last_command(ids: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let uniq = unique_ids(ids);

    if uniq.is_empty() {
        return Ok(());
    }

    let mut parts = Vec::new();

    for wid in uniq {
        if let Some(text) = preview::get_last_command_output(&wid) {
            if text.trim().is_empty() {
                continue;
            }

            parts.push(format!("# window {wid}\n{}", text.trim_end()));
        }
    }

    if parts.is_empty() {
        kitty_client::kitty_notify("WINSEL: nothing to yank");
        return Ok(());
    }

    let payload = parts.join("\n\n");
    copy_to_clipboard(&payload)?;
    kitty_client::kitty_notify("WINSEL: yanked to clipboard");

    Ok(())
}

fn move_selected(
    ids: &[String],
    focus_new_tab: bool,
    return_tab_id: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if ids.is_empty() {
        return Ok(());
    }

    // Move first window to a new tab.
    let first = ids[0].clone();
    let first_target = format!("id:{first}");
    let stay_in_tab = !focus_new_tab;
    kitty_client::kitty_first_ok(|mut cmd| {
        cmd.args(["detach-window", "--match", &first_target]);
        if stay_in_tab {
            cmd.arg("--stay-in-tab");
        }
        cmd.args(["--target-tab", "new"]);
        cmd.status()
    });

    // Find that new tab id (retry briefly in case kitty hasn't updated yet).
    let tab_id = find_tab_id_with_retry(&first)
        .ok_or_else(|| std::io::Error::other("cannot find new tab id"))?;
    let tab_match = format!("id:{tab_id}");
    utils::log_msg(format!("move-selected target tab id: {}", tab_id));

    // Move remaining windows into that tab.
    for wid in ids.iter().skip(1) {
        let target = format!("id:{wid}");
        kitty_client::kitty_first_ok(|mut cmd| {
            cmd.args(["detach-window", "--match", &target]);
            if stay_in_tab {
                cmd.arg("--stay-in-tab");
            }
            cmd.args(["--target-tab", &tab_match]);
            cmd.status()
        });
    }

    // Focus the new tab if requested, otherwise return to the caller tab.
    if focus_new_tab {
        kitty_client::kitty_first_ok(|mut cmd| {
            cmd.args(["focus-tab", "--match", &tab_match]);
            cmd.status()
        });
    } else if let Some(tab_id) = return_tab_id {
        focus_tab_by_id(tab_id);
    } else {
        focus_previous_tab();
    }

    Ok(())
}

fn focus_window(win_id: &str) {
    if win_id.is_empty() {
        return;
    }

    let target = format!("id:{win_id}");
    kitty_client::kitty_first_ok(|mut cmd| {
        cmd.args(["focus-window", "--match", &target]);
        cmd.status()
    });
}

fn focus_tab_by_id(tab_id: &str) {
    if tab_id.is_empty() {
        return;
    }
    let target = format!("id:{tab_id}");
    kitty_client::kitty_first_ok(|mut cmd| {
        cmd.args(["focus-tab", "--match", &target]);
        cmd.status()
    });
}

fn focus_previous_tab() {
    kitty_client::kitty_first_ok(|mut cmd| {
        cmd.args(["focus-tab", "--match", "recent:1"]);
        cmd.status()
    });
}

fn find_or_create_bg_tab() -> Result<String, Box<dyn std::error::Error>> {
    let mut data = kitty_client::kitty_ls(None)?;
    let os_id = active_os_window_id(&data)
        .or_else(|| find_active_os_window_id_fallback(&data))
        .ok_or_else(|| std::io::Error::other("no active OS window"))?;

    if let Some(id) = find_tab_id_by_title(&data, &os_id, "BG") {
        return Ok(id);
    }

    // Create BG tab without stealing focus.
    kitty_client::kitty_first_ok(|mut cmd| {
        cmd.args([
            "launch",
            "--type",
            "tab",
            "--tab-title",
            "BG",
            "--title",
            "BG",
            "--keep-focus",
        ]);
        cmd.status()
    });

    for attempt in 0..5 {
        data = kitty_client::kitty_ls(None)?;
        if let Some(id) = find_tab_id_by_title(&data, &os_id, "BG") {
            return Ok(id);
        }
        sleep(Duration::from_millis(30 * (attempt + 1) as u64));
    }

    Err(Box::new(std::io::Error::other(
        "unable to create BG tab",
    )))
}

fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut child = Command::new("kitty");
    child.args(["+kitten", "clipboard"]);
    child.stdin(Stdio::piped());

    let mut child = child.spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(Box::new(std::io::Error::other(
            "clipboard command failed",
        )));
    }
    Ok(())
}

fn find_tab_title(data: &kitty_api::KittyData, tab_id: &str) -> Option<String> {
    for oswin in &data.0 {
        for tab in &oswin.tabs {
            if tab.id.to_string() == tab_id {
                return Some(tab.title.clone());
            }
        }
    }
    None
}

fn active_os_window_id(data: &kitty_api::KittyData) -> Option<String> {
    for oswin in &data.0 {
        if oswin.is_focused {
            return Some(oswin.id.to_string());
        }
    }
    None
}

fn find_active_os_window_id_fallback(data: &kitty_api::KittyData) -> Option<String> {
    for oswin in &data.0 {
        if oswin.last_focused {
            return Some(oswin.id.to_string());
        }
    }
    data.0.first().map(|oswin| oswin.id.to_string())
}

fn find_tab_id_by_title(data: &kitty_api::KittyData, os_id: &str, title: &str) -> Option<String> {
    for oswin in &data.0 {
        if oswin.id.to_string() != os_id {
            continue;
        }
        for tab in &oswin.tabs {
            if tab.title.eq_ignore_ascii_case(title) {
                return Some(tab.id.to_string());
            }
        }
    }
    None
}

fn find_tab_id_for_window(data: &kitty_api::KittyData, win_id: &str) -> Option<String> {
    for oswin in &data.0 {
        for tab in &oswin.tabs {
            for win in &tab.windows {
                if win.id.to_string() == win_id {
                    return Some(tab.id.to_string());
                }
            }
        }
    }
    None
}

fn first_tab_id(data: &kitty_api::KittyData) -> Option<String> {
    data.0
        .first()
        .and_then(|oswin| oswin.tabs.first())
        .map(|tab| tab.id.to_string())
}

fn find_tab_id_with_retry(win_id: &str) -> Option<String> {
    for attempt in 0..5 {
        if let Ok(data) = kitty_client::kitty_ls(None) {
            if let Some(tab_id) = find_tab_id_for_window(&data, win_id) {
                return Some(tab_id);
            }
        }
        sleep(Duration::from_millis(20 * (attempt + 1) as u64));
    }
    None
}

fn find_recent_tab_id(idx: usize) -> Option<String> {
    let expr = format!("recent:{idx}");
    let out = kitty_client::kitty_first_ok(|mut cmd| {
        cmd.args(["ls", "--match-tab", &expr]);
        cmd.output()
    })?;

    let data = serde_json::from_slice::<kitty_api::KittyData>(&out.stdout).ok()?;
    first_tab_id(&data)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_preview_output_order() {
        // Simulate window content with numbered lines
        let simulated_body = (1..=100).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");

        // Simulate what tail_text does: get last max_lines
        let max_lines = 10;
        let lines: Vec<&str> = simulated_body.lines().rev().take(max_lines).collect();

        // Print in normal order (oldest to newest), then fzf "follow" scrolls to bottom
        let output: Vec<&str> = lines.into_iter().rev().collect();

        // Verify output is in normal order: oldest (Line 91) first, newest (Line 100) last
        assert_eq!(output[0], "Line 91");
        assert_eq!(output[1], "Line 92");
        assert_eq!(output[9], "Line 100");

        // Verify we got exactly max_lines
        assert_eq!(output.len(), max_lines);
    }

    #[test]
    fn test_preview_output_short_content() {
        // Test with content shorter than max_lines
        let simulated_body = (1..=5).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");

        let max_lines = 10;
        let lines: Vec<&str> = simulated_body.lines().rev().take(max_lines).collect();
        let output: Vec<&str> = lines.into_iter().rev().collect();

        // Should have all 5 lines in normal order: oldest first, newest last
        assert_eq!(output.len(), 5);
        assert_eq!(output[0], "Line 1");
        assert_eq!(output[4], "Line 5");
    }

    #[test]
    fn test_preview_shows_last_line() {
        // This test documents the critical behavior: the last line of output
        // should be the most recent content, which fzf "follow" will scroll to show.
        let simulated_body = (1..=200).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");

        let max_lines = 50;
        let lines: Vec<&str> = simulated_body.lines().rev().take(max_lines).collect();
        let output: Vec<&str> = lines.into_iter().rev().collect();

        // The last line in the output must be the most recent line from the window
        assert_eq!(output.len(), max_lines);
        assert_eq!(output[0], "Line 151"); // First line is oldest of the tail
        assert_eq!(output[49], "Line 200"); // Last line is the most recent

        // With fzf "follow" flag, this ensures Line 200 is visible at the bottom
    }
}
