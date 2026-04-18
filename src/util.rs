use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthChar;

pub fn run_output<S, I, A>(program: S, args: I, timeout_hint: Option<u64>) -> Option<String>
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    let _ = timeout_hint;
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() && out.stdout.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
        || run_output("which", [cmd], None).is_some()
}

pub fn format_memory(rss_kb: u64) -> String {
    if rss_kb > 1_048_576 {
        format!("{:.1} GB", rss_kb as f64 / 1_048_576.0)
    } else if rss_kb > 1024 {
        format!("{:.1} MB", rss_kb as f64 / 1024.0)
    } else {
        format!("{rss_kb} KB")
    }
}

pub fn format_uptime_from_seconds(seconds: u64) -> String {
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    if days > 0 {
        format!("{}d {}h", days, hours % 24)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes % 60)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

pub fn format_uptime_from_lstart(label: &str) -> Option<String> {
    let start = parse_ps_lstart(label)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    if now < start {
        return None;
    }
    Some(format_uptime_from_seconds((now - start) as u64))
}

fn parse_ps_lstart(label: &str) -> Option<i64> {
    let parts: Vec<&str> = label.split_whitespace().collect();
    let (mon, day, time, year) = match parts.as_slice() {
        [_, mon, day, time, year] => (*mon, *day, *time, *year),
        [mon, day, time, year] => (*mon, *day, *time, *year),
        _ => return None,
    };
    let month = match mon {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let day: i64 = day.parse().ok()?;
    let year: i64 = year.parse().ok()?;
    let t: Vec<i64> = time.split(':').filter_map(|v| v.parse().ok()).collect();
    if t.len() != 3 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + t[0] * 3600 + t[1] * 60 + t[2])
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = y - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn basename(s: &str) -> String {
    Path::new(s)
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| s.to_string())
}

pub fn path_basename(path: &Path) -> Option<String> {
    path.file_name().map(|v| v.to_string_lossy().to_string())
}

pub fn truncate_visible(s: &str, max: usize) -> String {
    if visible_width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let ellipsis_width = UnicodeWidthChar::width('…').unwrap_or(1);
    if max <= ellipsis_width {
        return "…".to_string();
    }

    let mut width = 0;
    let mut out = String::new();
    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width + ellipsis_width > max {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out.push('…');
    out
}

pub fn visible_width(s: &str) -> usize {
    s.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

pub fn find_project_root(dir: &Path) -> PathBuf {
    let markers = [
        "package.json",
        "Cargo.toml",
        "go.mod",
        "pyproject.toml",
        "Gemfile",
        "pom.xml",
        "build.gradle",
    ];
    let original = dir.to_path_buf();
    let mut current = dir.to_path_buf();
    for _ in 0..15 {
        if markers.iter().any(|m| current.join(m).exists()) {
            return current;
        }
        if !current.pop() {
            break;
        }
    }
    original
}

pub fn prompt_line(prompt: &str) -> Option<String> {
    use std::io::{self, Write};
    print!("{prompt}");
    io::stdout().flush().ok()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer).ok()?;
    Some(answer.trim_end_matches(['\r', '\n']).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn memory_format_uses_reference_thresholds() {
        assert_eq!(format_memory(1024), "1024 KB");
        assert_eq!(format_memory(1025), "1.0 MB");
        assert_eq!(format_memory(1_048_576), "1024.0 MB");
        assert_eq!(format_memory(1_048_577), "1.0 GB");
    }

    #[test]
    fn uptime_format_matches_reference_buckets() {
        assert_eq!(format_uptime_from_seconds(59), "59s");
        assert_eq!(format_uptime_from_seconds(61), "1m 1s");
        assert_eq!(format_uptime_from_seconds(3_661), "1h 1m");
        assert_eq!(format_uptime_from_seconds(90_000), "1d 1h");
    }

    #[test]
    fn project_root_walks_up_to_known_markers() {
        let root = temp_dir("root");
        let nested = root.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("go.mod"), "module example\n").unwrap();
        assert_eq!(find_project_root(&nested), root);
        fs::remove_dir_all(find_project_root(&nested)).unwrap();
    }

    #[test]
    fn truncation_uses_unicode_ellipsis() {
        assert_eq!(truncate_visible("abcdef", 4), "abc…");
        assert_eq!(truncate_visible("abc", 4), "abc");
    }

    #[test]
    fn truncation_uses_visible_width_for_wide_unicode() {
        assert_eq!(truncate_visible("表表表A", 5), "表表…");
        assert_eq!(truncate_visible("表A", 3), "表A");
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "port-whisperer-{label}-{}-{nanos}",
            std::process::id()
        ))
    }
}
