use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthChar;
use wait_timeout::ChildExt;

use crate::error::{PortError, record_user_warning, verbose_log_port_error};

pub fn run_output<S, I, A>(
    program: S,
    args: I,
    timeout: Option<Duration>,
) -> Result<String, PortError>
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    run_output_with_user_warnings(program, args, timeout, true)
}

#[cfg(unix)]
pub fn run_output_with_c_locale<S, I, A>(
    program: S,
    args: I,
    timeout: Option<Duration>,
) -> Result<String, PortError>
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    run_output_with_env(program, args, timeout, true, [("LANG", "C"), ("LC_ALL", "C")])
}

fn run_output_silent_probe<S, I, A>(
    program: S,
    args: I,
    timeout: Option<Duration>,
) -> Result<String, PortError>
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    run_output_with_user_warnings(program, args, timeout, false)
}

fn run_output_with_env<S, I, A, K, V, E>(
    program: S,
    args: I,
    timeout: Option<Duration>,
    record_warning: bool,
    envs: E,
) -> Result<String, PortError>
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
    E: IntoIterator<Item = (K, V)>,
{
    let program = program.as_ref().to_os_string();
    let args: Vec<std::ffi::OsString> = args
        .into_iter()
        .map(|a| a.as_ref().to_os_string())
        .collect();
    let envs: Vec<(std::ffi::OsString, std::ffi::OsString)> = envs
        .into_iter()
        .map(|(key, value)| (key.as_ref().to_os_string(), value.as_ref().to_os_string()))
        .collect();
    let result = run_output_impl(program, args, timeout, &envs);
    if let Err(ref e) = result {
        verbose_log_port_error(e);
        if record_warning {
            record_user_warning(e);
        }
    }
    result
}

fn run_output_with_user_warnings<S, I, A>(
    program: S,
    args: I,
    timeout: Option<Duration>,
    record_warning: bool,
) -> Result<String, PortError>
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    run_output_with_env(
        program,
        args,
        timeout,
        record_warning,
        std::iter::empty::<(&str, &str)>(),
    )
}

fn run_output_impl(
    program: std::ffi::OsString,
    args: Vec<std::ffi::OsString>,
    timeout: Option<Duration>,
    envs: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Result<String, PortError> {
    let cmd_str = format_cmd(&program, &args);
    let program_name = program.to_string_lossy().to_string();

    let mut command = Command::new(&program);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }

    let mut child = command
        .spawn()
        .map_err(|e| classify_spawn_error(e, &program_name))?;

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stdout_handle {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stderr_handle {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });

    let status = match timeout {
        Some(d) => match child.wait_timeout(d).map_err(PortError::Io)? {
            Some(s) => s,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(PortError::Timeout {
                    cmd: cmd_str,
                    ms: d.as_millis() as u64,
                });
            }
        },
        None => child.wait().map_err(PortError::Io)?,
    };

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    let stdout_str = String::from_utf8_lossy(&stdout).trim().to_string();
    let stderr_str = String::from_utf8_lossy(&stderr).trim().to_string();

    if status.success() || !stdout_str.is_empty() {
        Ok(stdout_str)
    } else {
        Err(PortError::ExecFailed {
            cmd: cmd_str,
            exit: status.code().unwrap_or(-1),
            stderr: stderr_str,
        })
    }
}

fn classify_spawn_error(e: std::io::Error, program: &str) -> PortError {
    match e.kind() {
        std::io::ErrorKind::NotFound => PortError::CommandMissing(program.to_string()),
        std::io::ErrorKind::PermissionDenied => PortError::PermissionDenied(program.to_string()),
        _ => PortError::Io(e),
    }
}

fn format_cmd(program: &OsStr, args: &[std::ffi::OsString]) -> String {
    let mut s = program.to_string_lossy().to_string();
    for arg in args {
        s.push(' ');
        s.push_str(&arg.to_string_lossy());
    }
    s
}

pub fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
        || run_output_silent_probe("which", [cmd], None).is_ok()
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
    use crate::error::PortError;
    use std::fs;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

    #[cfg(unix)]
    #[test]
    fn unix_ps_commands_are_wrapped_with_lang_c() {
        let out = run_output_with_c_locale(
            "sh",
            ["-c", "printf '%s/%s' \"$LANG\" \"$LC_ALL\""],
            None,
        )
        .expect("should succeed");

        assert_eq!(out, "C/C");
    }

    #[cfg(unix)]
    #[test]
    fn run_output_returns_ok_for_successful_command() {
        let out = run_output("echo", ["hello"], None).expect("should succeed");
        assert_eq!(out, "hello");
    }

    #[cfg(unix)]
    #[test]
    fn run_output_reports_command_missing_for_nonexistent_binary() {
        use crate::error::{drain_user_warnings, verbose_test_lock};
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();

        let err = run_output("definitely-not-a-real-command-xyz", ["arg"], None)
            .expect_err("should fail");
        assert!(matches!(err, PortError::CommandMissing(_)));
        drain_user_warnings();
    }

    #[cfg(unix)]
    #[test]
    fn run_output_err_writes_to_verbose_log_when_enabled() {
        use crate::error::{drain_verbose_log, set_verbose_enabled, verbose_test_lock};
        let _guard = verbose_test_lock().lock().unwrap();
        set_verbose_enabled(true);
        drain_verbose_log();

        let _ = run_output("definitely-not-a-real-command-xyz-2", ["arg"], None);
        let drained = drain_verbose_log();
        set_verbose_enabled(false);

        assert!(
            drained
                .iter()
                .any(|m| m.contains("not found")
                    && m.contains("definitely-not-a-real-command-xyz-2")),
            "verbose log should capture full PortError message, got {drained:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_output_err_does_not_accumulate_when_verbose_disabled() {
        use crate::error::{drain_verbose_log, set_verbose_enabled, verbose_test_lock};
        let _guard = verbose_test_lock().lock().unwrap();
        set_verbose_enabled(false);
        drain_verbose_log();

        let _ = run_output("definitely-not-a-real-command-xyz-3", ["arg"], None);
        assert!(drain_verbose_log().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn run_output_err_records_user_warning_even_when_verbose_disabled() {
        use crate::error::{drain_user_warnings, set_verbose_enabled, verbose_test_lock};
        let _guard = verbose_test_lock().lock().unwrap();
        set_verbose_enabled(false);
        drain_user_warnings();

        let _ = run_output("definitely-not-a-real-command-xyz-4", ["arg"], None);
        let drained = drain_user_warnings();

        assert_eq!(drained.len(), 1);
        assert_eq!(
            drained[0].user_message(),
            "definitely-not-a-real-command-xyz-4 not found; results may be incomplete"
        );
    }

    #[cfg(unix)]
    #[test]
    fn command_exists_probe_does_not_record_user_warning() {
        use crate::error::{drain_user_warnings, set_verbose_enabled, verbose_test_lock};
        let _guard = verbose_test_lock().lock().unwrap();
        set_verbose_enabled(false);
        drain_user_warnings();

        assert!(!command_exists("definitely-not-a-real-command-xyz-5"));
        assert!(drain_user_warnings().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn run_output_times_out_for_long_running_child() {
        use crate::error::{drain_user_warnings, verbose_test_lock};
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();

        let result = run_output("sleep", ["10"], Some(Duration::from_millis(200)));
        assert!(
            matches!(result, Err(PortError::Timeout { .. })),
            "expected Timeout, got {:?}",
            result
        );
        drain_user_warnings();
    }

    #[cfg(unix)]
    #[test]
    fn run_output_timeout_reports_command_text_and_requested_ms() {
        use crate::error::{drain_user_warnings, verbose_test_lock};
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();

        let result = run_output("sleep", ["10"], Some(Duration::from_millis(200)))
            .expect_err("sleep should time out");

        match result {
            PortError::Timeout { cmd, ms } => {
                assert!(cmd.contains("sleep"), "cmd={cmd:?}");
                assert!(cmd.contains("10"), "cmd={cmd:?}");
                assert_eq!(ms, 200);
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
        drain_user_warnings();
    }

    #[cfg(unix)]
    #[test]
    fn run_output_timeout_returns_well_before_command_natural_exit() {
        use crate::error::{drain_user_warnings, verbose_test_lock};
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();

        let start = Instant::now();
        let result = run_output("sleep", ["10"], Some(Duration::from_millis(200)));
        let elapsed = start.elapsed();

        assert!(matches!(result, Err(PortError::Timeout { .. })));
        assert!(
            elapsed < Duration::from_secs(3),
            "timeout path should finish well before natural exit: {elapsed:?}"
        );
        drain_user_warnings();
    }

    #[cfg(unix)]
    #[test]
    fn run_output_reports_exec_failed_on_nonzero_exit_with_no_stdout() {
        use crate::error::{drain_user_warnings, verbose_test_lock};
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();

        let err = run_output("sh", ["-c", "echo oops >&2; exit 3"], None).expect_err("should fail");
        match err {
            PortError::ExecFailed { exit, stderr, .. } => {
                assert_eq!(exit, 3);
                assert!(stderr.contains("oops"), "stderr={stderr:?}");
            }
            other => panic!("expected ExecFailed, got {other:?}"),
        }
        drain_user_warnings();
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
