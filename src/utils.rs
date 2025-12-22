use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

/// Compute elapsed time in seconds from a timestamp
/// Handles timestamps in nanoseconds, milliseconds, or seconds
pub fn compute_elapsed(start_ns: i128) -> Option<i128> {
    if start_ns <= 0 {
        return None;
    }

    // Normalize timestamp to seconds based on magnitude
    // > 2 billion billion = nanoseconds (kitty uses this)
    // > 2 billion = milliseconds
    // otherwise = seconds
    let start_secs = if start_ns > 2_000_000_000_000 {
        start_ns as f64 / 1_000_000_000f64 // nanoseconds to seconds
    } else if start_ns > 2_000_000_000 {
        start_ns as f64 / 1_000f64 // milliseconds to seconds
    } else {
        start_ns as f64 // already in seconds
    };

    let now = SystemTime::UNIX_EPOCH
        .elapsed()
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let diff = (now - start_secs).max(0.0);
    Some(diff as i128)
}

/// Format seconds into human-readable duration (e.g., "1d 2h 3m 4s")
pub fn human_time(seconds: Option<i128>) -> String {
    let sec = match seconds {
        Some(s) => s,
        None => return "n/a".to_string(),
    };

    let mut rem = sec;
    let days = rem / 86_400;
    rem %= 86_400;
    let hours = rem / 3_600;
    rem %= 3_600;
    let minutes = rem / 60;
    let seconds = rem % 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    parts.push(format!("{seconds}s"));
    parts.join(" ")
}

/// Remove tabs and newlines from text, trim whitespace
pub fn clean(input: &str) -> String {
    input
        .replace(['\t', '\n'], " ")
        .trim()
        .to_string()
}

/// Abbreviate path by replacing $HOME with ~
pub fn abbreviate_path(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }

    if let Ok(home) = env::var("HOME") {
        return path.replace(&home, "~");
    }

    path.to_string()
}

/// Log a message to /tmp/winsel.log
pub fn log_msg(msg: impl AsRef<str>) {
    let path = "/tmp/winsel.log";
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", msg.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // compute_elapsed tests
    #[test]
    fn test_compute_elapsed_nanoseconds() {
        // Use current time for a very recent timestamp (should be ~0 seconds elapsed)
        let now_secs = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_secs() as i128;
        let nanos = now_secs * 1_000_000_000; // Convert to nanoseconds
        let elapsed = compute_elapsed(nanos);
        assert!(elapsed.is_some());
        // Should be very small (within a few seconds of now)
        let secs = elapsed.unwrap();
        assert!(secs >= 0);
        assert!(secs < 10); // Should be less than 10 seconds
    }

    #[test]
    fn test_compute_elapsed_milliseconds() {
        let millis = 1700000000000i128; // milliseconds
        let elapsed = compute_elapsed(millis);
        assert!(elapsed.is_some());
        let secs = elapsed.unwrap();
        assert!(secs >= 0);
    }

    #[test]
    fn test_compute_elapsed_seconds() {
        let secs_timestamp = 1700000000i128; // seconds
        let elapsed = compute_elapsed(secs_timestamp);
        assert!(elapsed.is_some());
        let secs = elapsed.unwrap();
        assert!(secs >= 0);
    }

    #[test]
    fn test_compute_elapsed_zero() {
        assert_eq!(compute_elapsed(0), None);
    }

    #[test]
    fn test_compute_elapsed_negative() {
        assert_eq!(compute_elapsed(-100), None);
    }

    // human_time tests
    #[test]
    fn test_human_time_none() {
        assert_eq!(human_time(None), "n/a");
    }

    #[test]
    fn test_human_time_seconds_only() {
        assert_eq!(human_time(Some(45)), "45s");
    }

    #[test]
    fn test_human_time_minutes_and_seconds() {
        assert_eq!(human_time(Some(125)), "2m 5s"); // 2*60 + 5
    }

    #[test]
    fn test_human_time_hours_minutes_seconds() {
        assert_eq!(human_time(Some(3665)), "1h 1m 5s"); // 1*3600 + 1*60 + 5
    }

    #[test]
    fn test_human_time_full() {
        assert_eq!(human_time(Some(90061)), "1d 1h 1m 1s"); // 1*86400 + 1*3600 + 1*60 + 1
    }

    #[test]
    fn test_human_time_zero() {
        assert_eq!(human_time(Some(0)), "0s");
    }

    #[test]
    fn test_human_time_exact_hour() {
        assert_eq!(human_time(Some(3600)), "1h 0s");
    }

    #[test]
    fn test_human_time_exact_day() {
        assert_eq!(human_time(Some(86400)), "1d 0s");
    }

    // clean tests
    #[test]
    fn test_clean_tabs() {
        assert_eq!(clean("hello\tworld"), "hello world");
    }

    #[test]
    fn test_clean_newlines() {
        assert_eq!(clean("hello\nworld"), "hello world");
    }

    #[test]
    fn test_clean_multiple_whitespace() {
        // Note: clean() replaces tabs/newlines with spaces but doesn't collapse multiple spaces
        assert_eq!(clean("hello\t\nworld"), "hello  world");
    }

    #[test]
    fn test_clean_trim() {
        assert_eq!(clean("  hello  "), "hello");
    }

    #[test]
    fn test_clean_combination() {
        // Note: clean() replaces tabs/newlines with spaces but doesn't collapse multiple spaces
        assert_eq!(clean("  \thello\n\tworld  "), "hello  world");
    }

    #[test]
    fn test_clean_empty() {
        assert_eq!(clean(""), "");
    }

    #[test]
    fn test_clean_whitespace_only() {
        assert_eq!(clean("  \t\n  "), "");
    }

    // abbreviate_path tests
    #[test]
    fn test_abbreviate_path_empty() {
        assert_eq!(abbreviate_path(""), "");
    }

    #[test]
    fn test_abbreviate_path_with_home() {
        // This test depends on $HOME being set
        if let Ok(home) = env::var("HOME") {
            let full_path = format!("{}/documents", home);
            assert_eq!(abbreviate_path(&full_path), "~/documents");
        }
    }

    #[test]
    fn test_abbreviate_path_without_home() {
        assert_eq!(abbreviate_path("/usr/local/bin"), "/usr/local/bin");
    }

    #[test]
    fn test_abbreviate_path_exact_home() {
        if let Ok(home) = env::var("HOME") {
            assert_eq!(abbreviate_path(&home), "~");
        }
    }

    #[test]
    fn test_abbreviate_path_home_multiple_occurrences() {
        // Should replace all occurrences
        if let Ok(home) = env::var("HOME") {
            let path = format!("{}/path/to/{}/file", home, home);
            let expected = format!("~/path/to/~/file");
            assert_eq!(abbreviate_path(&path), expected);
        }
    }

    // log_msg test - just verify it doesn't panic
    #[test]
    fn test_log_msg_does_not_panic() {
        log_msg("test message");
        log_msg(String::from("another test"));
    }
}
