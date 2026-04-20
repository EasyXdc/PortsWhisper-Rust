use crate::error::PortError;
use crate::model::{KillResolutionKind, LogFdKind, LogFile};
use crate::scanner;
use crate::style;
use crate::util::{prompt_line, run_output};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct LogsRequest {
    follow: bool,
    err_only: bool,
    lines: String,
    targets: Vec<String>,
}

pub fn run_logs(args: &[String]) -> i32 {
    let parsed = parse_logs_request(args);
    if parsed.targets.is_empty() {
        for line in logs_usage_lines() {
            println!("{line}");
        }
        return 1;
    }
    let Ok(target) = parsed.targets[0].parse::<u32>() else {
        println!(
            "{}",
            style::red(format!(
                "\n  ✕ \"{}\" is not a valid port/PID\n",
                parsed.targets[0]
            ))
        );
        return 1;
    };
    let Some(resolved) = scanner::resolve_kill_target(target) else {
        let msg = if target <= 65_535 {
            format!("No listener on :{target} and no process with PID {target}")
        } else {
            format!("No process with PID {target}")
        };
        println!("{}", style::red(format!("\n  ✕ {msg}\n")));
        return 1;
    };
    let port_label = match resolved.via {
        KillResolutionKind::Port => format!(":{}", resolved.port.unwrap_or(target as u16)),
        KillResolutionKind::Pid => format!("PID {}", resolved.pid),
    };
    let process_name = resolved
        .info
        .as_ref()
        .map(|p| p.process_name.clone())
        .unwrap_or_else(|| "unknown".to_string());
    println!();
    for line in log_header_lines(&port_label, resolved.pid, &process_name) {
        println!("{line}");
    }
    println!();

    let log_files = get_process_log_files(resolved.pid);
    match select_log_file(&log_files, parsed.err_only) {
        LogSelection::Tail(file) if parsed.err_only => {
            println!(
                "  {} Tailing stderr: {}\n",
                style::yellow("▸"),
                style::dim(file.path.to_string_lossy())
            );
            return tail_file(&file.path, &parsed.lines, parsed.follow);
        }
        LogSelection::NoStderr => {
            println!(
                "{}",
                style::yellow(format!(
                    "  No stderr redirect found for PID {}\n",
                    resolved.pid
                ))
            );
            return 0;
        }
        _ => {}
    }

    match select_log_file(&log_files, false) {
        LogSelection::Tail(file) => {
            let label = match file.fd {
                LogFdKind::Stdout => "stdout",
                LogFdKind::Stderr => "stderr",
                LogFdKind::File => "log",
            };
            println!(
                "  {} Tailing {}: {}\n",
                style::green("▸"),
                label,
                style::dim(file.path.to_string_lossy())
            );
            return tail_file(&file.path, &parsed.lines, parsed.follow);
        }
        LogSelection::NeedsUserSelection => {}
        LogSelection::NoFiles => {}
        LogSelection::NoStderr => unreachable!("err_only is false"),
    }

    if log_files.len() > 1 {
        println!("{}", style::bold("  Found log files:\n"));
        for (idx, file) in log_files.iter().enumerate() {
            let label = match file.fd {
                LogFdKind::Stdout => style::green("stdout"),
                LogFdKind::Stderr => style::yellow("stderr"),
                LogFdKind::File => style::dim(&file.kind),
            };
            println!(
                "    {}  {}  {}",
                style::white_bold((idx + 1).to_string()),
                label,
                style::dim(file.path.to_string_lossy())
            );
        }
        println!();
        let selection = choose_log_file_index(
            prompt_line(&style::yellow(format!(
                "  Pick a file (1-{}): ",
                log_files.len()
            )))
            .as_deref(),
            log_files.len(),
        );
        let selected = match selection {
            LogSelectionChoice::Selected(idx) => &log_files[idx],
            LogSelectionChoice::Cancelled => return 1,
            LogSelectionChoice::Invalid => {
                println!("{}", style::red("\n  Invalid selection.\n"));
                return 0;
            }
        };
        println!(
            "\n  {} Tailing: {}\n",
            style::green("▸"),
            style::dim(selected.path.to_string_lossy())
        );
        return tail_file(&selected.path, &parsed.lines, parsed.follow);
    }

    if let Some(sys_cmd) = get_system_log_command(resolved.pid, parsed.follow) {
        println!(
            "{}",
            style::yellow("  No log files found. Falling back to system log...\n")
        );
        println!("  {}\n", style::dim(format!("$ {sys_cmd}")));
        return run_shell(&sys_cmd);
    }
    println!(
        "{}",
        style::yellow(format!(
            "  No log files or system log found for PID {}.\n",
            resolved.pid
        ))
    );
    println!(
        "{}",
        style::dim(
            "  Tip: if the process logs to the terminal, check the terminal where it was started.\n"
        )
    );
    0
}

fn parse_logs_request(args: &[String]) -> LogsRequest {
    let follow = args.iter().any(|a| a == "-f" || a == "--follow");
    let err_only = args.iter().any(|a| a == "--err");
    let lines = parse_lines(args).to_string();
    let targets = args
        .iter()
        .skip(1)
        .filter(|a| {
            let s = a.as_str();
            s != "-f"
                && s != "--follow"
                && s != "--err"
                && s != "--lines"
                && !s.starts_with("--lines=")
                && s != lines
        })
        .cloned()
        .collect();
    LogsRequest {
        follow,
        err_only,
        lines,
        targets,
    }
}

pub fn get_process_log_files(pid: u32) -> Vec<LogFile> {
    let mut files = Vec::new();
    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        files.extend(log_files_from_lsof_result(run_output(
            "lsof",
            ["-p", &pid.to_string()],
            Some(std::time::Duration::from_millis(5000)),
        )));
        if files.is_empty() && cfg!(target_os = "linux") {
            let fd_dir = PathBuf::from(format!("/proc/{pid}/fd"));
            if let Ok(entries) = std::fs::read_dir(fd_dir) {
                for entry in entries.flatten() {
                    let fd_name = entry.file_name().to_string_lossy().to_string();
                    if let Ok(target) = std::fs::read_link(entry.path()) {
                        let target_s = target.to_string_lossy();
                        if fd_name == "1"
                            && !target_s.starts_with("/dev/")
                            && !target_s.starts_with("pipe:")
                        {
                            files.push(LogFile {
                                path: target,
                                fd: LogFdKind::Stdout,
                                kind: "redirect".to_string(),
                                priority: 1,
                            });
                        } else if fd_name == "2"
                            && !target_s.starts_with("/dev/")
                            && !target_s.starts_with("pipe:")
                        {
                            files.push(LogFile {
                                path: target,
                                fd: LogFdKind::Stderr,
                                kind: "redirect".to_string(),
                                priority: 1,
                            });
                        } else if is_log_like_path(&target_s) {
                            files.push(LogFile {
                                path: target,
                                fd: LogFdKind::File,
                                kind: "logfile".to_string(),
                                priority: 2,
                            });
                        }
                    }
                }
            }
        }
    }

    if let Some(cwd) = get_process_cwd(pid) {
        for rel in [
            ".next/server.log",
            "logs/development.log",
            "log/development.log",
            "tmp/pids/server.log",
            "storage/logs/laravel.log",
            "npm-debug.log",
            "yarn-error.log",
        ] {
            let full = cwd.join(rel);
            if full.exists() {
                files.push(LogFile {
                    path: full,
                    fd: LogFdKind::File,
                    kind: "framework".to_string(),
                    priority: 3,
                });
            }
        }
    }
    sort_and_dedupe_log_files(files)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LogSelection {
    Tail(LogFile),
    NeedsUserSelection,
    NoStderr,
    NoFiles,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LogSelectionChoice {
    Selected(usize),
    Invalid,
    Cancelled,
}

pub(crate) fn select_log_file(log_files: &[LogFile], err_only: bool) -> LogSelection {
    if err_only {
        return log_files
            .iter()
            .find(|f| f.fd == LogFdKind::Stderr)
            .cloned()
            .map(LogSelection::Tail)
            .unwrap_or(LogSelection::NoStderr);
    }
    match log_files {
        [] => LogSelection::NoFiles,
        [file] => LogSelection::Tail(file.clone()),
        _ => LogSelection::NeedsUserSelection,
    }
}

pub(crate) fn choose_log_file_index(answer: Option<&str>, len: usize) -> LogSelectionChoice {
    let Some(answer) = answer else {
        return LogSelectionChoice::Cancelled;
    };
    let Ok(idx) = answer.trim().parse::<usize>() else {
        return LogSelectionChoice::Invalid;
    };
    if idx == 0 || idx > len {
        return LogSelectionChoice::Invalid;
    }
    LogSelectionChoice::Selected(idx - 1)
}

pub(crate) fn sort_and_dedupe_log_files(mut files: Vec<LogFile>) -> Vec<LogFile> {
    files.sort_by_key(|f| f.priority);
    let mut seen = HashSet::new();
    files
        .into_iter()
        .filter(|f| seen.insert(f.path.clone()))
        .collect()
}

pub(crate) fn is_log_like_path(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".log")
        || lower.contains("/log/")
        || lower.contains("/logs/")
        || lower.contains("\\log\\")
        || lower.contains("\\logs\\")
        || lower.contains("/tmp/")
        || lower.contains("nohup.out")
        || lower.contains("stdout")
        || lower.contains("stderr")
}

fn get_process_cwd(pid: u32) -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
    } else if cfg!(target_os = "macos") {
        get_process_cwd_from_lsof_result(run_output(
            "lsof",
            ["-p", &pid.to_string(), "-d", "cwd", "-Fn"],
            Some(std::time::Duration::from_millis(3000)),
        ))
    } else if cfg!(target_os = "windows") {
        run_output(
            "powershell",
            [
                "-Command",
                &format!("(Get-Process -Id {pid}).Path | Split-Path"),
            ],
            Some(std::time::Duration::from_millis(5000)),
        )
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
    } else {
        None
    }
}

fn log_files_from_lsof_result(result: Result<String, PortError>) -> Vec<LogFile> {
    let mut files = Vec::new();
    let raw = result.ok().unwrap_or_default();
    for line in raw.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 9 {
            continue;
        }
        let fd = cols[3];
        let kind = cols[4];
        let name = cols[8..].join(" ");
        if (fd == "1w" || fd == "2w") && kind == "REG" {
            files.push(LogFile {
                path: PathBuf::from(name),
                fd: if fd == "1w" {
                    LogFdKind::Stdout
                } else {
                    LogFdKind::Stderr
                },
                kind: "redirect".to_string(),
                priority: 1,
            });
        } else if kind == "REG" && fd.ends_with('w') && is_log_like_path(&name) {
            files.push(LogFile {
                path: PathBuf::from(name),
                fd: LogFdKind::File,
                kind: "logfile".to_string(),
                priority: 2,
            });
        }
    }
    files
}

fn get_process_cwd_from_lsof_result(result: Result<String, PortError>) -> Option<PathBuf> {
    let raw = result.ok().unwrap_or_default();
    raw.lines()
        .find(|l| l.starts_with('n'))
        .map(|l| PathBuf::from(&l[1..]))
}

pub fn get_system_log_command(pid: u32, follow: bool) -> Option<String> {
    if cfg!(target_os = "macos") {
        Some(if follow {
            format!("log stream --predicate 'processID == {pid}' --style compact")
        } else {
            format!("log show --predicate 'processID == {pid}' --style compact --last 1m")
        })
    } else if cfg!(target_os = "linux") {
        Some(if follow {
            format!("journalctl _PID={pid} -f --no-pager")
        } else {
            format!("journalctl _PID={pid} --no-pager -n 50")
        })
    } else if cfg!(target_os = "windows") {
        Some(format!(
            "powershell -Command \"Get-WinEvent -FilterHashtable @{{LogName='Application'; ProcessId={pid}}} -MaxEvents 50\""
        ))
    } else {
        None
    }
}

pub(crate) fn parse_lines(args: &[String]) -> &str {
    for arg in args {
        if let Some(v) = arg.strip_prefix("--lines=") {
            return v;
        }
    }
    for pair in args.windows(2) {
        if pair[0] == "--lines" {
            return &pair[1];
        }
    }
    "50"
}

fn logs_usage_lines() -> [String; 3] {
    [
        style::red("\n  Usage: ports logs <port|pid> [-f] [--lines=N] [--err]\n"),
        style::gray("  Show log output for a process running on a port."),
        style::gray("  Use -f or --follow to stream new lines.\n"),
    ]
}

fn log_header_lines(port_label: &str, pid: u32, process_name: &str) -> [String; 1] {
    [format!(
        "{}{}",
        style::cyan_bold("  Port Whisperer"),
        style::gray(format!(
            " — logs for {port_label} ({process_name}, PID {pid})"
        ))
    )]
}

#[cfg(test)]
mod tests {
    use super::{
        LogSelection, LogSelectionChoice, TailCommand, build_tail_command, choose_log_file_index,
        get_process_cwd_from_lsof_result, is_log_like_path, log_files_from_lsof_result,
        log_header_lines, logs_usage_lines, parse_lines, parse_logs_request, select_log_file,
        sort_and_dedupe_log_files,
    };
    use crate::error::PortError;
    use crate::model::{LogFdKind, LogFile};
    use std::path::PathBuf;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn parses_lines_equals_and_space_forms() {
        assert_eq!(parse_lines(&args(&["logs", "3000", "--lines=10"])), "10");
        assert_eq!(parse_lines(&args(&["logs", "3000", "--lines", "25"])), "25");
        assert_eq!(parse_lines(&args(&["logs", "3000"])), "50");
    }

    #[test]
    fn detects_log_like_paths_from_reference_patterns() {
        assert!(is_log_like_path("/app/log/development.log"));
        assert!(is_log_like_path("/app/logs/server.txt"));
        assert!(is_log_like_path("/tmp/stdout"));
        assert!(is_log_like_path("nohup.out"));
        assert!(is_log_like_path(r"C:\app\logs\stderr.txt"));
        assert!(!is_log_like_path("/app/src/main.rs"));
    }

    #[test]
    fn sorts_log_files_by_priority_and_deduplicates_paths() {
        let files = vec![
            log_file("/app/log/server.log", LogFdKind::File, "logfile", 2),
            log_file("/app/out.log", LogFdKind::File, "framework", 3),
            log_file("/app/out.log", LogFdKind::Stdout, "redirect", 1),
            log_file("/app/err.log", LogFdKind::Stderr, "redirect", 1),
        ];
        let normalized = sort_and_dedupe_log_files(files);
        let paths: Vec<_> = normalized
            .iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            paths,
            vec!["/app/out.log", "/app/err.log", "/app/log/server.log"]
        );
        assert_eq!(normalized[0].fd, LogFdKind::Stdout);
    }

    #[test]
    fn fake_log_files_drive_log_selection_without_tailing() {
        let stdout = log_file("/app/out.log", LogFdKind::Stdout, "redirect", 1);
        let stderr = log_file("/app/err.log", LogFdKind::Stderr, "redirect", 1);
        assert_eq!(
            select_log_file(std::slice::from_ref(&stdout), false),
            LogSelection::Tail(stdout.clone())
        );
        assert_eq!(
            select_log_file(&[stdout.clone(), stderr.clone()], false),
            LogSelection::NeedsUserSelection
        );
        assert_eq!(
            select_log_file(&[stdout.clone(), stderr.clone()], true),
            LogSelection::Tail(stderr)
        );
        assert_eq!(select_log_file(&[stdout], true), LogSelection::NoStderr);
        assert_eq!(select_log_file(&[], false), LogSelection::NoFiles);
    }

    #[test]
    fn logs_messages_include_usage_and_header_fields() {
        let usage = logs_usage_lines();
        assert!(usage[0].contains("Usage: ports logs <port|pid> [-f] [--lines=N] [--err]"));
        assert!(usage[1].contains("Show log output for a process running on a port."));
        assert!(usage[2].contains("Use -f or --follow to stream new lines."));

        let header = log_header_lines(":3000", 42, "node");
        assert!(header[0].contains("Port Whisperer"));
        assert!(header[0].contains("logs for :3000"));
        assert!(header[0].contains("node, PID 42"));
    }

    #[test]
    fn multi_file_selection_accepts_valid_choice_and_rejects_invalid_input() {
        assert_eq!(
            choose_log_file_index(Some("2"), 3),
            LogSelectionChoice::Selected(1)
        );
        assert_eq!(
            choose_log_file_index(Some("abc"), 3),
            LogSelectionChoice::Invalid
        );
        assert_eq!(
            choose_log_file_index(Some("0"), 3),
            LogSelectionChoice::Invalid
        );
        assert_eq!(
            choose_log_file_index(Some("4"), 3),
            LogSelectionChoice::Invalid
        );
        assert_eq!(
            choose_log_file_index(None, 3),
            LogSelectionChoice::Cancelled
        );
    }

    #[test]
    fn follow_mode_uses_streaming_tail_command() {
        let follow = build_tail_command(&PathBuf::from("/app/server.log"), "25", true);
        let tail = build_tail_command(&PathBuf::from("/app/server.log"), "25", false);

        if cfg!(target_os = "windows") {
            assert!(matches!(follow, TailCommand::PowerShell { .. }));
            let TailCommand::PowerShell { command } = follow else {
                unreachable!()
            };
            assert!(command.contains("-Tail 25 -Wait"));

            let TailCommand::PowerShell { command } = tail else {
                unreachable!()
            };
            assert!(command.contains("-Tail 25"));
            assert!(!command.contains("-Wait"));
        } else {
            assert_eq!(
                follow,
                TailCommand::Argv(vec![
                    "tail".to_string(),
                    "-f".to_string(),
                    "-n".to_string(),
                    "25".to_string(),
                    "/app/server.log".to_string(),
                ])
            );
            assert_eq!(
                tail,
                TailCommand::Argv(vec![
                    "tail".to_string(),
                    "-n".to_string(),
                    "25".to_string(),
                    "/app/server.log".to_string(),
                ])
            );
        }
    }

    #[test]
    fn logs_argument_parsing_matches_node_reference_behavior() {
        let parsed = parse_logs_request(&args(&["logs", "3000", "--lines=5", "-f", "--err"]));

        assert_eq!(parsed.targets, vec!["3000".to_string()]);
        assert_eq!(parsed.lines, "5");
        assert!(parsed.follow);
        assert!(parsed.err_only);
    }

    #[test]
    fn lsof_timeout_degrades_to_empty_log_discovery_result() {
        let files = log_files_from_lsof_result(Err(PortError::Timeout {
            cmd: "lsof -p 42".to_string(),
            ms: 5000,
        }));

        assert!(files.is_empty());
    }

    #[test]
    fn get_process_cwd_timeout_returns_none() {
        let cwd = get_process_cwd_from_lsof_result(Err(PortError::Timeout {
            cmd: "lsof -p 42 -d cwd -Fn".to_string(),
            ms: 3000,
        }));

        assert_eq!(cwd, None);
    }

    fn log_file(path: &str, fd: LogFdKind, kind: &str, priority: u8) -> LogFile {
        LogFile {
            path: PathBuf::from(path),
            fd,
            kind: kind.to_string(),
            priority,
        }
    }
}

fn tail_file(path: &Path, lines: &str, follow: bool) -> i32 {
    let status = match build_tail_command(path, lines, follow) {
        TailCommand::PowerShell { command } => Command::new("powershell")
            .args(["-Command", &command])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status(),
        TailCommand::Argv(argv) => {
            let mut cmd = Command::new(&argv[0]);
            cmd.args(&argv[1..])
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
        }
    };
    if status.map(|s| s.success()).unwrap_or(false) {
        0
    } else {
        1
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TailCommand {
    PowerShell { command: String },
    Argv(Vec<String>),
}

pub(crate) fn build_tail_command(path: &Path, lines: &str, follow: bool) -> TailCommand {
    if cfg!(target_os = "windows") {
        let wait = if follow { " -Wait" } else { "" };
        TailCommand::PowerShell {
            command: format!(
                "Get-Content -Path '{}' -Tail {}{}",
                path.to_string_lossy(),
                lines,
                wait
            ),
        }
    } else {
        let mut argv = vec!["tail".to_string()];
        if follow {
            argv.push("-f".to_string());
        }
        argv.push("-n".to_string());
        argv.push(lines.to_string());
        argv.push(path.to_string_lossy().to_string());
        TailCommand::Argv(argv)
    }
}

fn run_shell(cmd: &str) -> i32 {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", cmd])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    } else {
        Command::new("sh")
            .args(["-c", cmd])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    };
    if status.map(|s| s.success()).unwrap_or(false) {
        0
    } else {
        1
    }
}
