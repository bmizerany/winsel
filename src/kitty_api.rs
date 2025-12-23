use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

// Helper to deserialize a field that can be either a string or an array of strings
fn deserialize_string_or_seq_option<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    use serde_json::Value;

    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(arr) => arr
            .into_iter()
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .ok_or_else(|| Error::custom("expected string in array"))
            })
            .collect(),
        Value::String(s) => {
            // Split string on whitespace to create array
            Ok(s.split_whitespace().map(String::from).collect())
        }
        Value::Null => Ok(vec![]),
        _ => Ok(vec![]), // Default to empty on unexpected type
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct KittyData(pub Vec<OsWindow>);

#[derive(Debug, Deserialize, Clone)]
pub struct OsWindow {
    pub id: i64,
    pub is_focused: bool,
    #[serde(default)]
    pub last_focused: bool,
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Tab {
    pub id: i64,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_focused: bool,
    pub title: String,
    #[serde(default)]
    pub active_window_history: Vec<i64>,
    pub windows: Vec<Window>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Window {
    pub id: i64,
    #[serde(default)]
    pub is_focused: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default, deserialize_with = "deserialize_string_or_seq_option")]
    pub last_reported_cmdline: Vec<String>,
    #[serde(default)]
    pub foreground_processes: Vec<Process>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub user_vars: HashMap<String, String>,
}

impl Window {
    /// Get the command line for this window, preferring last_reported_cmdline
    /// over foreground_processes[0].cmdline
    pub fn cmdline(&self) -> &[String] {
        if !self.last_reported_cmdline.is_empty() {
            &self.last_reported_cmdline
        } else if let Some(proc) = self.foreground_processes.first() {
            &proc.cmdline
        } else {
            &[]
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Process {
    #[serde(default)]
    pub cmdline: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_full_data() {
        let json = r#"[{
            "id": 1,
            "is_focused": true,
            "last_focused": false,
            "tabs": [{
                "id": 1,
                "is_active": true,
                "is_focused": true,
                "title": "Main",
                "active_window_history": [1, 2],
                "windows": [{
                    "id": 1,
                    "is_focused": true,
                    "title": "vim",
                    "cwd": "/home/user",
                    "created_at": 1700000000000000000,
                    "last_reported_cmdline": ["vim", "file.txt"],
                    "foreground_processes": []
                }]
            }]
        }]"#;

        let data: KittyData = serde_json::from_str(json).unwrap();
        assert_eq!(data.0.len(), 1);
        assert_eq!(data.0[0].id, 1);
        assert!(data.0[0].is_focused);
        assert_eq!(data.0[0].tabs.len(), 1);
        assert_eq!(data.0[0].tabs[0].windows.len(), 1);

        let window = &data.0[0].tabs[0].windows[0];
        assert_eq!(window.id, 1);
        assert_eq!(window.title, "vim");
        assert_eq!(window.cwd, "/home/user");
        assert_eq!(window.created_at, 1700000000000000000);
        assert_eq!(window.last_reported_cmdline, vec!["vim", "file.txt"]);
    }

    #[test]
    fn test_deserialize_missing_optional_fields() {
        let json = r#"[{
            "id": 1,
            "is_focused": true,
            "tabs": [{
                "id": 1,
                "title": "Main",
                "windows": [{
                    "id": 1
                }]
            }]
        }]"#;

        let data: KittyData = serde_json::from_str(json).unwrap();
        assert_eq!(data.0.len(), 1);

        let window = &data.0[0].tabs[0].windows[0];
        assert_eq!(window.id, 1);
        assert!(!window.is_focused); // default false
        assert_eq!(window.title, ""); // default empty string
        assert_eq!(window.cwd, "");
        assert_eq!(window.created_at, 0);
        assert!(window.last_reported_cmdline.is_empty());
        assert!(window.foreground_processes.is_empty());
    }

    #[test]
    fn test_deserialize_empty_arrays() {
        let json = r#"[]"#;
        let data: KittyData = serde_json::from_str(json).unwrap();
        assert_eq!(data.0.len(), 0);
    }

    #[test]
    fn test_window_cmdline_prefers_last_reported() {
        let window = Window {
            id: 1,
            is_focused: false,
            title: String::new(),
            cwd: String::new(),
            created_at: 0,
            last_reported_cmdline: vec!["vim".to_string(), "file.txt".to_string()],
            foreground_processes: vec![Process {
                cmdline: vec!["bash".to_string()],
            }],
            env: HashMap::new(),
            user_vars: HashMap::new(),
        };

        assert_eq!(window.cmdline(), &["vim", "file.txt"]);
    }

    #[test]
    fn test_window_cmdline_fallback_to_foreground() {
        let window = Window {
            id: 1,
            is_focused: false,
            title: String::new(),
            cwd: String::new(),
            created_at: 0,
            last_reported_cmdline: vec![],
            foreground_processes: vec![Process {
                cmdline: vec!["bash".to_string()],
            }],
            env: HashMap::new(),
            user_vars: HashMap::new(),
        };

        assert_eq!(window.cmdline(), &["bash"]);
    }

    #[test]
    fn test_window_cmdline_empty_fallback() {
        let window = Window {
            id: 1,
            is_focused: false,
            title: String::new(),
            cwd: String::new(),
            created_at: 0,
            last_reported_cmdline: vec![],
            foreground_processes: vec![],
            env: HashMap::new(),
            user_vars: HashMap::new(),
        };

        assert_eq!(window.cmdline(), &[] as &[String]);
    }

    #[test]
    fn test_malformed_json() {
        let json = r#"{"not": "an array"}"#;
        let result: Result<KittyData, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_real_kitty_output() {
        // Real output from `kitty @ ls` - tests last_reported_cmdline as string
        let json = r#"[{
            "id": 18,
            "is_focused": true,
            "last_focused": true,
            "tabs": [{
                "id": 247,
                "is_active": true,
                "is_focused": true,
                "title": "Testing JSON Parsing",
                "active_window_history": [404, 367, 403],
                "windows": [{
                    "id": 367,
                    "is_focused": true,
                    "title": "Testing JSON Parsing",
                    "cwd": "/Users/bmizerany/",
                    "created_at": 1766392476039566000,
                    "last_reported_cmdline": "claude --dangerously-skip-permissions ",
                    "foreground_processes": [{
                        "cmdline": ["claude", "--dangerously-skip-permissions"]
                    }]
                }]
            }]
        }]"#;

        let data: KittyData = serde_json::from_str(json).unwrap();
        assert_eq!(data.0.len(), 1);
        assert_eq!(data.0[0].id, 18);
        assert_eq!(data.0[0].tabs.len(), 1);

        let window = &data.0[0].tabs[0].windows[0];
        assert_eq!(window.id, 367);
        // last_reported_cmdline is a string in real output, gets split on whitespace
        assert_eq!(window.last_reported_cmdline, vec!["claude", "--dangerously-skip-permissions"]);
        assert_eq!(window.cmdline(), &["claude", "--dangerously-skip-permissions"]);
    }
}
