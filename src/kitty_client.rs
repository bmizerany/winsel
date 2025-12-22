use crate::{kitty_api, utils};
use std::env;
use std::process::Command;

/// Get kitty window list data
pub fn kitty_ls(
    match_id: Option<&str>,
) -> Result<kitty_api::KittyData, Box<dyn std::error::Error>> {
    let mut last_err: Option<Box<dyn std::error::Error>> = None;

    for mut cmd in kitty_commands() {
        utils::log_msg(format!("kitty ls try: {:?}", cmd));
        cmd.arg("ls");
        if let Some(mid) = match_id {
            cmd.args(["--match", &format!("id:{mid}")]);
        }

        match cmd.output() {
            Ok(out) => match serde_json::from_slice::<kitty_api::KittyData>(&out.stdout) {
                Ok(data) => {
                    utils::log_msg("kitty ls success");
                    return Ok(data);
                }
                Err(e) => {
                    utils::log_msg(format!("kitty ls decode error: {e}"));
                    last_err = Some(Box::new(e))
                }
            },
            Err(e) => {
                utils::log_msg(format!("kitty ls exec error: {e}"));
                last_err = Some(Box::new(e))
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        Box::new(std::io::Error::other("kitty ls failed for all targets"))
    }))
}

/// Try kitty commands until one succeeds (generic helper)
pub fn kitty_first_ok<F, T>(mut f: F) -> Option<T>
where
    F: FnMut(Command) -> std::io::Result<T>,
{
    for cmd in kitty_commands() {
        match f(cmd) {
            Ok(val) => return Some(val),
            Err(_) => continue,
        }
    }
    None
}

/// Send notification via kitty
pub fn kitty_notify(msg: &str) {
    for mut cmd in kitty_commands() {
        if cmd.args(["kitten", "notify", msg]).status().is_ok() {
            break;
        }
    }
}

/// Create base kitty command
fn kitty_base_cmd() -> Command {
    let mut cmd = Command::new("kitty");
    cmd.arg("@");
    cmd
}

/// Get all possible kitty command variations to try (with different sockets)
fn kitty_commands() -> Vec<Command> {
    let mut cmds = Vec::new();
    let candidates = [
        env::var("KITTY_LISTEN_ON").ok(),
        env::var("KITTY_SOCKET").ok(),
        Some("unix:/tmp/kitty".to_string()),
        Some("unix:@kitty".to_string()),
        None,
    ];

    for cand in candidates.into_iter().flatten() {
        if cand.is_empty() {
            continue;
        }
        let mut cmd = kitty_base_cmd();
        cmd.args(["--to", &cand]);
        cmds.push(cmd);
    }

    // final fallback: no explicit target
    cmds.push(kitty_base_cmd());
    cmds
}
