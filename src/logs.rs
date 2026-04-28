#[cfg(any(target_os = "linux", target_os = "macos", test))]
use crate::error::PortError;
use crate::json_output;
use crate::model::{KillResolutionKind, LogFdKind, LogFile, TailCommand};
use crate::scanner;
use crate::style;
use crate::util::prompt_line;
use std::collections::HashSet;
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "macos", test))]
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct LogsRequest {
    follow: bool,
    err_only: bool,
    lines: String,
    grep: Option<String>,
    since: Option<String>,
    targets: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
enum LogsRequestError {
    MissingValue(&'static str),
    InvalidValue(&'static str),
}

#[derive(Debug, Eq, PartialEq)]
struct LogsJsonResult {
    payload: json_output::LogsPayload,
    exit_code: i32,
}

#[derive(Debug, Eq, PartialEq)]
struct LogsJsonError {
    code: String,
    message: String,
    exit_code: i32,
}

pub fn run_logs(args: &[String]) -> i32 {
    let glyphs = style::glyphs();
    let parsed = match parse_logs_request(args) {
        Ok(parsed) => parsed,
        Err(LogsRequestError::MissingValue(flag)) => {
            println!(
                "{}",
                style::red(format!("\n  {} missing value for {flag}\n", glyphs.failure))
            );
            return 1;
        }
        Err(LogsRequestError::InvalidValue(flag)) => {
            println!(
                "{}",
                style::red(format!("\n  {} invalid value for {flag}\n", glyphs.failure))
            );
            return 1;
        }
    };
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
                "\n  {} \"{}\" is not a valid port/PID\n",
                glyphs.failure, parsed.targets[0]
            ))
        );
        return 1;
    };
    let Some(resolved) = scanner::resolve_kill_target(target) else {
        let msg = if crate::model::is_likely_port(target) {
            format!("No listener on :{target} and no process with PID {target}")
        } else {
            format!("No process with PID {target}")
        };
        println!("{}", style::red(format!("\n  {} {msg}\n", glyphs.failure)));
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
                style::yellow(glyphs.logs_pointer),
                style::dim(file.path.to_string_lossy())
            );
            return tail_file(
                &file.path,
                &parsed.lines,
                parsed.follow,
                parsed.grep.as_deref(),
            );
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
                style::green(glyphs.logs_pointer),
                label,
                style::dim(file.path.to_string_lossy())
            );
            return tail_file(
                &file.path,
                &parsed.lines,
                parsed.follow,
                parsed.grep.as_deref(),
            );
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
            style::green(glyphs.logs_pointer),
            style::dim(selected.path.to_string_lossy())
        );
        return tail_file(
            &selected.path,
            &parsed.lines,
            parsed.follow,
            parsed.grep.as_deref(),
        );
    }

    if parsed.follow && !crate::platform::native_scanner().supports_system_log_follow() {
        println!(
            "{}",
            style::red(format!(
                "\n  {} follow mode is not supported for system logs on this platform\n",
                glyphs.failure
            ))
        );
        return 1;
    }

    if let Some(sys_cmd) =
        get_system_log_command_with_since(resolved.pid, parsed.follow, parsed.since.as_deref())
    {
        println!(
            "{}",
            style::yellow("  No log files found. Falling back to system log...\n")
        );
        println!("  {}\n", style::dim(format!("$ {sys_cmd}")));
        return run_shell(&apply_grep_to_shell_command(
            &sys_cmd,
            parsed.grep.as_deref(),
        ));
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

pub fn run_logs_json(args: &[String]) -> i32 {
    let command = format!("ports {}", args.join(" "));
    let result = run_logs_json_with(
        args,
        scanner::resolve_kill_target,
        get_process_log_files,
        read_log_output,
        get_system_log_command_with_since,
        run_shell_output,
    );
    let (json_result, exit_code) = match result {
        Ok(r) => {
            let exit_code = r.exit_code;
            (render_logs_ok_json(&command, r), exit_code)
        }
        Err(e) => {
            let exit_code = e.exit_code;
            (render_logs_err_json(&command, e), exit_code)
        }
    };
    let json_exit = json_output::print_json_output(json_result);
    if json_exit != 0 { json_exit } else { exit_code }
}

fn render_logs_ok_json(command: &str, result: LogsJsonResult) -> serde_json::Result<String> {
    let warnings = crate::error::drain_user_warnings()
        .into_iter()
        .map(|warning| warning.user_message())
        .collect::<Vec<_>>();
    json_output::render_json(
        &json_output::CommandEnvelope::ok(command, result.payload).with_warnings(warnings),
    )
}

fn render_logs_err_json(command: &str, err: LogsJsonError) -> serde_json::Result<String> {
    let warnings = crate::error::drain_user_warnings()
        .into_iter()
        .map(|warning| warning.user_message())
        .collect::<Vec<_>>();
    json_output::render_json(
        &json_output::CommandEnvelope::<json_output::LogsPayload>::err(
            command,
            err.code,
            err.message,
        )
        .with_warnings(warnings),
    )
}

#[cfg(test)]
fn render_logs_json_result(
    command: &str,
    result: Result<LogsJsonResult, LogsJsonError>,
) -> serde_json::Result<(String, i32)> {
    match result {
        Ok(result) => {
            let exit_code = result.exit_code;
            render_logs_ok_json(command, result).map(|output| (output, exit_code))
        }
        Err(err) => {
            let exit_code = err.exit_code;
            render_logs_err_json(command, err).map(|output| (output, exit_code))
        }
    }
}

fn parse_logs_request(args: &[String]) -> Result<LogsRequest, LogsRequestError> {
    let follow = args.iter().any(|a| a == "-f" || a == "--follow");
    let err_only = args.iter().any(|a| a == "--err");
    let lines = parse_lines(args).to_string();
    let grep = parse_flag_value(args, "--grep")?.map(ToOwned::to_owned);
    let since = parse_flag_value(args, "--since")?.map(ToOwned::to_owned);
    if since
        .as_deref()
        .is_some_and(|value| !valid_since_value(value))
    {
        return Err(LogsRequestError::InvalidValue("--since"));
    }
    let mut targets = Vec::new();
    let mut iter = args.iter().skip(1).peekable();
    while let Some(arg) = iter.next() {
        let s = arg.as_str();
        if s == "-f"
            || s == "--follow"
            || s == "--err"
            || s.starts_with("--lines=")
            || s.starts_with("--grep=")
            || s.starts_with("--since=")
        {
            continue;
        }
        if s == "--lines" || s == "--grep" || s == "--since" {
            iter.next();
            continue;
        }
        targets.push(arg.clone());
    }
    Ok(LogsRequest {
        follow,
        err_only,
        lines,
        grep,
        since,
        targets,
    })
}

fn valid_since_value(value: &str) -> bool {
    !value.is_empty() && !value.starts_with('-') && crate::model::is_shell_safe(value)
}

fn run_logs_json_with<Resolve, Discover, ReadLog, SystemCmd, RunSystem>(
    args: &[String],
    resolve: Resolve,
    discover_log_files: Discover,
    read_log: ReadLog,
    system_log_command: SystemCmd,
    run_system_log: RunSystem,
) -> Result<LogsJsonResult, LogsJsonError>
where
    Resolve: Fn(u32) -> Option<crate::model::KillTargetResolution>,
    Discover: Fn(u32) -> Vec<LogFile>,
    ReadLog: Fn(&Path, &str, bool) -> Result<String, String>,
    SystemCmd: Fn(u32, bool, Option<&str>) -> Option<String>,
    RunSystem: Fn(&str) -> Result<String, String>,
{
    let parsed = parse_logs_request(args).map_err(|err| match err {
        LogsRequestError::MissingValue(flag) => LogsJsonError {
            code: "usage".to_string(),
            message: format!("missing value for {flag}"),
            exit_code: 1,
        },
        LogsRequestError::InvalidValue(flag) => LogsJsonError {
            code: "usage".to_string(),
            message: format!("invalid value for {flag}"),
            exit_code: 1,
        },
    })?;
    if parsed.follow {
        return Err(LogsJsonError {
            code: "unsupported_follow".to_string(),
            message: "follow mode is not supported with --json for ports logs <port|pid> [-f] [--lines=N] [--err] [--grep <pattern>] [--since <value>] yet".to_string(),
            exit_code: 1,
        });
    }
    if parsed.targets.is_empty() {
        return Err(LogsJsonError {
            code: "usage".to_string(),
            message: "Usage: ports logs <port|pid> [-f] [--lines=N] [--err] [--grep <pattern>] [--since <value>]".to_string(),
            exit_code: 1,
        });
    }
    let Ok(target) = parsed.targets[0].parse::<u32>() else {
        return Err(LogsJsonError {
            code: "invalid_target".to_string(),
            message: format!("\"{}\" is not a valid port/PID", parsed.targets[0]),
            exit_code: 1,
        });
    };
    let Some(resolved) = resolve(target) else {
        let message = if target <= 65_535 {
            format!("No listener on :{target} and no process with PID {target}")
        } else {
            format!("No process with PID {target}")
        };
        return Err(LogsJsonError {
            code: "target_not_found".to_string(),
            message,
            exit_code: 1,
        });
    };

    let process_name = resolved
        .info
        .as_ref()
        .map(|p| p.process_name.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let log_files = discover_log_files(resolved.pid);

    if parsed.err_only {
        return match select_log_file(&log_files, true) {
            LogSelection::Tail(file) => Ok(LogsJsonResult {
                payload: json_output::logs_payload(
                    resolved.pid,
                    resolved.port,
                    process_name,
                    false,
                    parsed.lines.clone(),
                    true,
                    Some(log_source_payload(&file, None)),
                    Some(apply_grep_filter(
                        read_log(&file.path, &parsed.lines, false).map_err(|message| {
                            LogsJsonError {
                                code: "log_read_failed".to_string(),
                                message,
                                exit_code: 1,
                            }
                        })?,
                        parsed.grep.as_deref(),
                    )),
                ),
                exit_code: 0,
            }),
            LogSelection::NoStderr => Ok(LogsJsonResult {
                payload: json_output::logs_payload(
                    resolved.pid,
                    resolved.port,
                    process_name,
                    false,
                    parsed.lines,
                    true,
                    None,
                    None,
                ),
                exit_code: 0,
            }),
            _ => unreachable!("stderr selection should not require user choice"),
        };
    }

    match select_log_file(&log_files, false) {
        LogSelection::Tail(file) => Ok(LogsJsonResult {
            payload: json_output::logs_payload(
                resolved.pid,
                resolved.port,
                process_name,
                false,
                parsed.lines.clone(),
                false,
                Some(log_source_payload(&file, None)),
                Some(apply_grep_filter(
                    read_log(&file.path, &parsed.lines, false).map_err(|message| {
                        LogsJsonError {
                            code: "log_read_failed".to_string(),
                            message,
                            exit_code: 1,
                        }
                    })?,
                    parsed.grep.as_deref(),
                )),
            ),
            exit_code: 0,
        }),
        LogSelection::NeedsUserSelection => Err(LogsJsonError {
            code: "multiple_log_files".to_string(),
            message: "multiple log files found; interactive selection is not supported with --json"
                .to_string(),
            exit_code: 1,
        }),
        LogSelection::NoFiles => {
            if let Some(cmd) = system_log_command(resolved.pid, false, parsed.since.as_deref()) {
                return Ok(LogsJsonResult {
                    payload: json_output::logs_payload(
                        resolved.pid,
                        resolved.port,
                        process_name,
                        false,
                        parsed.lines,
                        false,
                        Some(json_output::LogSourcePayload {
                            kind: "system".to_string(),
                            path: None,
                            command: Some(cmd.clone()),
                        }),
                        Some(apply_grep_filter(
                            run_system_log(&cmd).map_err(|message| LogsJsonError {
                                code: "system_log_failed".to_string(),
                                message,
                                exit_code: 1,
                            })?,
                            parsed.grep.as_deref(),
                        )),
                    ),
                    exit_code: 0,
                });
            }
            Ok(LogsJsonResult {
                payload: json_output::logs_payload(
                    resolved.pid,
                    resolved.port,
                    process_name,
                    false,
                    parsed.lines,
                    false,
                    None,
                    None,
                ),
                exit_code: 0,
            })
        }
        LogSelection::NoStderr => unreachable!("err_only is false"),
    }
}

fn log_source_payload(file: &LogFile, command: Option<String>) -> json_output::LogSourcePayload {
    json_output::LogSourcePayload {
        kind: match file.fd {
            LogFdKind::Stdout => "stdout",
            LogFdKind::Stderr => "stderr",
            LogFdKind::File => "file",
        }
        .to_string(),
        path: Some(file.path.to_string_lossy().into_owned()),
        command,
    }
}

pub fn get_process_log_files(pid: u32) -> Vec<LogFile> {
    let scanner = crate::platform::native_scanner();
    let mut files = scanner.get_process_log_files(pid);

    if let Some(cwd) = scanner.get_cwd_for_pid(pid) {
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

#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(crate) fn log_files_from_lsof_result(result: Result<String, PortError>) -> Vec<LogFile> {
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

#[cfg(target_os = "linux")]
pub(crate) fn log_files_from_proc_fd(pid: u32) -> Vec<LogFile> {
    let mut files = Vec::new();
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
    files
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn merge_log_discovery_results(
    primary: Vec<LogFile>,
    fallback: Vec<LogFile>,
) -> Vec<LogFile> {
    let mut files = primary;
    files.extend(fallback);
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

#[cfg(any(target_os = "linux", target_os = "macos", test))]
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

#[cfg(test)]
fn get_process_cwd_from_lsof_result(result: Result<String, PortError>) -> Option<PathBuf> {
    let raw = result.ok().unwrap_or_default();
    raw.lines()
        .find(|l| l.starts_with('n'))
        .map(|l| PathBuf::from(&l[1..]))
}

pub fn get_system_log_command(pid: u32, follow: bool) -> Option<String> {
    get_system_log_command_with_since(pid, follow, None)
}

fn get_system_log_command_with_since(
    pid: u32,
    follow: bool,
    since: Option<&str>,
) -> Option<String> {
    crate::platform::native_scanner().get_system_log_command_with_since(pid, follow, since)
}

pub(crate) fn parse_lines(args: &[String]) -> &str {
    for arg in args {
        if let Some(v) = arg.strip_prefix("--lines=") {
            if v.parse::<u32>().is_ok() {
                return v;
            }
            return "50";
        }
    }
    for pair in args.windows(2) {
        if pair[0] == "--lines" {
            if pair[1].parse::<u32>().is_ok() {
                return &pair[1];
            }
            return "50";
        }
    }
    "50"
}

fn parse_flag_value<'a>(
    args: &'a [String],
    flag: &'static str,
) -> Result<Option<&'a str>, LogsRequestError> {
    for arg in args {
        if let Some(value) = arg.strip_prefix(&format!("{flag}=")) {
            return Ok(Some(value));
        }
    }
    for pair in args.windows(2) {
        if pair[0] == flag {
            if pair[1].starts_with('-') {
                return Err(LogsRequestError::MissingValue(flag));
            }
            return Ok(Some(&pair[1]));
        }
    }
    if args.last().map(String::as_str) == Some(flag) {
        return Err(LogsRequestError::MissingValue(flag));
    }
    Ok(None)
}

fn apply_grep_filter(output: String, pattern: Option<&str>) -> String {
    let Some(pattern) = pattern else {
        return output.trim_end_matches('\n').to_string();
    };
    output
        .lines()
        .filter(|line| line.contains(pattern))
        .collect::<Vec<_>>()
        .join("\n")
}

fn apply_grep_to_shell_command(cmd: &str, grep: Option<&str>) -> String {
    let Some(grep) = grep else {
        return cmd.to_string();
    };
    crate::platform::native_scanner().apply_grep_to_shell_command(cmd, grep)
}

fn logs_usage_lines() -> [String; 3] {
    [
        style::red(
            "\n  Usage: ports logs <port|pid> [-f] [--lines=N] [--err] [--grep <pattern>] [--since <value>]\n",
        ),
        style::gray("  Show log output for a process running on a port, with optional filtering."),
        style::gray(
            "  Use -f/--follow to stream, --grep to match lines, and --since for system logs.\n",
        ),
    ]
}

fn log_header_lines(port_label: &str, pid: u32, process_name: &str) -> [String; 1] {
    let glyphs = style::glyphs();
    [format!(
        "{} {}{}",
        style::cyan_bold(glyphs.logs_pointer),
        style::cyan_bold("  Port Whisperer"),
        style::gray(format!(
            " — logs for {port_label} ({process_name}, PID {pid})"
        ))
    )]
}

#[cfg(test)]
mod tests {
    use super::{
        LogSelection, LogSelectionChoice, LogsJsonError, LogsJsonResult, LogsRequestError,
        apply_grep_to_shell_command, build_tail_command, choose_log_file_index,
        get_process_cwd_from_lsof_result, is_log_like_path, log_files_from_lsof_result,
        log_header_lines, logs_usage_lines, merge_log_discovery_results, parse_lines,
        parse_logs_request, render_logs_json_result, run_logs_json_with, select_log_file,
        sort_and_dedupe_log_files, tail_follow_shell_command,
    };
    use crate::error::{PortError, drain_user_warnings, record_user_warning, verbose_test_lock};
    use crate::model::{KillResolutionKind, KillTargetResolution, LogFdKind, LogFile, TailCommand};
    use crate::test_support::fake_port;
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
    fn merge_log_discovery_results_prefers_proc_results_before_lsof_fallback() {
        let proc_files = vec![
            log_file("/app/out.log", LogFdKind::Stdout, "redirect", 1),
            log_file("/app/app.log", LogFdKind::File, "logfile", 2),
        ];
        let lsof_files = vec![
            log_file("/app/out.log", LogFdKind::File, "logfile", 2),
            log_file("/app/err.log", LogFdKind::Stderr, "redirect", 1),
        ];

        let merged = merge_log_discovery_results(proc_files, lsof_files);

        assert_eq!(
            merged,
            vec![
                log_file("/app/out.log", LogFdKind::Stdout, "redirect", 1),
                log_file("/app/err.log", LogFdKind::Stderr, "redirect", 1),
                log_file("/app/app.log", LogFdKind::File, "logfile", 2),
            ]
        );
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
        assert!(usage[0].contains("Usage: ports logs <port|pid> [-f] [--lines=N] [--err] [--grep <pattern>] [--since <value>]"));
        assert!(usage[1].contains("optional filtering"));
        assert!(usage[2].contains("--grep"));
        assert!(usage[2].contains("--since"));

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
        let parsed = parse_logs_request(&args(&["logs", "3000", "--lines=5", "-f", "--err"]))
            .expect("logs args should parse");

        assert_eq!(parsed.targets, vec!["3000".to_string()]);
        assert_eq!(parsed.lines, "5");
        assert!(parsed.follow);
        assert!(parsed.err_only);
        assert_eq!(parsed.grep, None);
        assert_eq!(parsed.since, None);
    }

    #[test]
    fn logs_argument_parsing_keeps_target_when_it_matches_lines_value() {
        let parsed = parse_logs_request(&args(&["logs", "3000", "--lines", "3000"]))
            .expect("logs args should parse");

        assert_eq!(parsed.targets, vec!["3000".to_string()]);
        assert_eq!(parsed.lines, "3000");
    }

    #[test]
    fn logs_argument_parsing_supports_grep_and_since_flags() {
        let parsed = parse_logs_request(&args(&[
            "logs", "3000", "--grep", "error", "--since", "10m",
        ]))
        .expect("logs args should parse");

        assert_eq!(parsed.targets, vec!["3000".to_string()]);
        assert_eq!(parsed.grep.as_deref(), Some("error"));
        assert_eq!(parsed.since.as_deref(), Some("10m"));
        assert_eq!(parsed.lines, "50");
    }

    #[test]
    fn logs_argument_parsing_rejects_missing_grep_and_since_values() {
        assert!(matches!(
            parse_logs_request(&args(&["logs", "3000", "--grep"])),
            Err(LogsRequestError::MissingValue("--grep"))
        ));
        assert!(matches!(
            parse_logs_request(&args(&["logs", "3000", "--since"])),
            Err(LogsRequestError::MissingValue("--since"))
        ));
        assert!(matches!(
            parse_logs_request(&args(&["logs", "3000", "--grep", "--since", "5m"])),
            Err(LogsRequestError::MissingValue("--grep"))
        ));
    }

    #[test]
    fn logs_argument_parsing_rejects_invalid_since_values() {
        assert!(matches!(
            parse_logs_request(&args(&["logs", "3000", "--since="])),
            Err(LogsRequestError::InvalidValue("--since"))
        ));
        assert!(matches!(
            parse_logs_request(&args(&["logs", "3000", "--since=--predicate"])),
            Err(LogsRequestError::InvalidValue("--since"))
        ));
        assert!(matches!(
            parse_logs_request(&args(&["logs", "3000", "--since=1h;rm"])),
            Err(LogsRequestError::InvalidValue("--since"))
        ));
    }

    #[test]
    fn grep_shell_command_wraps_streaming_plain_mode_commands() {
        let command = apply_grep_to_shell_command("tail -f -n 25 /app/server.log", Some("error"));

        if cfg!(target_os = "windows") {
            assert!(command.contains("Select-String -SimpleMatch"));
        } else {
            assert!(command.contains("grep --line-buffered -F -- \"error\""));
            assert!(command.starts_with("tail -f -n 25 /app/server.log | "));
        }
    }

    #[test]
    fn follow_mode_file_log_command_applies_grep_filter() {
        let command =
            tail_follow_shell_command(&PathBuf::from("/app/server.log"), "25", Some("error"));

        if cfg!(target_os = "windows") {
            assert!(command.contains("Get-Content -Path '/app/server.log' -Tail 25 -Wait"));
            assert!(command.contains("Select-String -SimpleMatch"));
        } else {
            assert!(command.contains("tail -f -n 25 /app/server.log"));
            assert!(command.contains("grep --line-buffered -F -- \"error\""));
        }
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

    #[test]
    fn json_logs_result_captures_non_follow_output_from_log_file() {
        let result = run_logs_json_with(
            &args(&["logs", "3000", "--lines=5"]),
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 42,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 42)),
                }),
                _ => None,
            },
            |_pid| vec![log_file("/app/out.log", LogFdKind::Stdout, "redirect", 1)],
            |_path, lines, follow| {
                assert_eq!(lines, "5");
                assert!(!follow);
                Ok("ready\nrequest /health".to_string())
            },
            |_pid, _follow, _since| None,
            |_cmd| unreachable!("system logs should not be used when a file exists"),
        )
        .expect("json logs should succeed");

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.payload.pid, 42);
        assert_eq!(result.payload.port, Some(3000));
        assert_eq!(result.payload.process_name, "node");
        assert!(!result.payload.follow);
        assert_eq!(result.payload.lines, "5");
        assert!(!result.payload.stderr_only);
        assert_eq!(
            result.payload.output.as_deref(),
            Some("ready\nrequest /health")
        );
        let source = result.payload.source.expect("log source should be present");
        assert_eq!(source.kind, "stdout");
        assert_eq!(source.path.as_deref(), Some("/app/out.log"));
        assert_eq!(source.command, None);
    }

    #[test]
    fn json_logs_applies_grep_filter_to_log_file_output() {
        let result = run_logs_json_with(
            &args(&["logs", "3000", "--grep", "error"]),
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 42,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 42)),
                }),
                _ => None,
            },
            |_pid| vec![log_file("/app/out.log", LogFdKind::Stdout, "redirect", 1)],
            |_path, _lines, _follow| {
                Ok("ready\nerror: failed\nrequest /health\nerror: retried".to_string())
            },
            |_pid, _follow, _since| None,
            |_cmd| unreachable!("system logs should not be used when a file exists"),
        )
        .expect("json logs should succeed");

        assert_eq!(
            result.payload.output.as_deref(),
            Some("error: failed\nerror: retried")
        );
    }

    #[test]
    fn json_logs_passes_since_filter_to_system_log_command() {
        let result = run_logs_json_with(
            &args(&["logs", "3000", "--since", "2h"]),
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 42,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 42)),
                }),
                _ => None,
            },
            |_pid| Vec::new(),
            |_path, _lines, _follow| unreachable!("file logs should not be used"),
            |_pid, _follow, _since| Some("system-log --since 2h".to_string()),
            |cmd| {
                assert_eq!(cmd, "system-log --since 2h");
                Ok("system line".to_string())
            },
        )
        .expect("system logs should succeed");

        assert_eq!(result.payload.output.as_deref(), Some("system line"));
        let source = result
            .payload
            .source
            .expect("system source should be present");
        assert_eq!(source.command.as_deref(), Some("system-log --since 2h"));
    }

    #[test]
    fn json_logs_follow_mode_returns_error_instead_of_text_fallback() {
        let err = run_logs_json_with(
            &args(&["logs", "3000", "-f"]),
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 42,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 42)),
                }),
                _ => None,
            },
            |_pid| vec![log_file("/app/out.log", LogFdKind::Stdout, "redirect", 1)],
            |_path, _lines, _follow| unreachable!("follow mode should be rejected before reading"),
            |_pid, _follow, _since| unreachable!("system command should not be consulted"),
            |_cmd| unreachable!("system command should not execute"),
        )
        .expect_err("follow mode should be rejected for json");

        assert_eq!(err.exit_code, 1);
        assert!(err.message.contains("follow"));
        assert!(err.message.contains("--json"));
        assert!(err.message.contains("--grep <pattern>"));
    }

    #[test]
    fn json_logs_missing_flag_values_return_usage_error() {
        let err = run_logs_json_with(
            &args(&["logs", "3000", "--grep"]),
            |_target| None,
            |_pid| Vec::new(),
            |_path, _lines, _follow| Ok(String::new()),
            |_pid, _follow, _since| None,
            |_cmd| Ok(String::new()),
        )
        .expect_err("missing grep value should fail");

        assert_eq!(err.code, "usage");
        assert_eq!(err.message, "missing value for --grep");
    }

    #[test]
    fn json_logs_invalid_since_returns_usage_error() {
        let err = run_logs_json_with(
            &args(&["logs", "3000", "--since=--predicate"]),
            |_target| None,
            |_pid| Vec::new(),
            |_path, _lines, _follow| Ok(String::new()),
            |_pid, _follow, _since| None,
            |_cmd| Ok(String::new()),
        )
        .expect_err("invalid since value should fail");

        assert_eq!(err.code, "usage");
        assert_eq!(err.message, "invalid value for --since");
    }

    #[test]
    fn json_logs_failure_paths_render_structured_error_envelopes() {
        let follow_json = render_logs_json_result(
            "ports logs 3000 -f",
            run_logs_json_with(
                &args(&["logs", "3000", "-f"]),
                |_target| None,
                |_pid| Vec::new(),
                |_path, _lines, _follow| Ok(String::new()),
                |_pid, _follow, _since| None,
                |_cmd| Ok(String::new()),
            ),
        )
        .expect("json should render");
        let invalid_json = render_logs_json_result(
            "ports logs abc",
            run_logs_json_with(
                &args(&["logs", "abc"]),
                |_target| None,
                |_pid| Vec::new(),
                |_path, _lines, _follow| Ok(String::new()),
                |_pid, _follow, _since| None,
                |_cmd| Ok(String::new()),
            ),
        )
        .expect("json should render");
        let unresolved_json = render_logs_json_result(
            "ports logs 3000",
            run_logs_json_with(
                &args(&["logs", "3000"]),
                |_target| None,
                |_pid| Vec::new(),
                |_path, _lines, _follow| Ok(String::new()),
                |_pid, _follow, _since| None,
                |_cmd| Ok(String::new()),
            ),
        )
        .expect("json should render");
        let multi_file_json = render_logs_json_result(
            "ports logs 3000",
            run_logs_json_with(
                &args(&["logs", "3000"]),
                |target| match target {
                    3000 => Some(KillTargetResolution {
                        pid: 42,
                        via: KillResolutionKind::Port,
                        port: Some(3000),
                        info: Some(fake_port(3000, 42)),
                    }),
                    _ => None,
                },
                |_pid| {
                    vec![
                        log_file("/app/out.log", LogFdKind::Stdout, "redirect", 1),
                        log_file("/app/err.log", LogFdKind::Stderr, "redirect", 1),
                    ]
                },
                |_path, _lines, _follow| Ok(String::new()),
                |_pid, _follow, _since| None,
                |_cmd| Ok(String::new()),
            ),
        )
        .expect("json should render");

        let follow =
            serde_json::from_str::<serde_json::Value>(&follow_json.0).expect("json should parse");
        let invalid =
            serde_json::from_str::<serde_json::Value>(&invalid_json.0).expect("json should parse");
        let unresolved = serde_json::from_str::<serde_json::Value>(&unresolved_json.0)
            .expect("json should parse");
        let multi_file = serde_json::from_str::<serde_json::Value>(&multi_file_json.0)
            .expect("json should parse");

        assert_eq!(follow["ok"], false);
        assert_eq!(follow["error"]["code"], "unsupported_follow");
        assert_eq!(invalid["ok"], false);
        assert_eq!(invalid["error"]["code"], "invalid_target");
        assert_eq!(unresolved["ok"], false);
        assert_eq!(unresolved["error"]["code"], "target_not_found");
        assert_eq!(multi_file["ok"], false);
        assert_eq!(multi_file["error"]["code"], "multiple_log_files");
    }

    #[test]
    fn logs_json_success_includes_drained_user_warnings() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        record_user_warning(&PortError::Timeout {
            cmd: "lsof -p 42".to_string(),
            ms: 3000,
        });

        let rendered = render_logs_json_result(
            "ports logs 3000",
            Ok(LogsJsonResult {
                payload: crate::json_output::logs_payload(
                    42,
                    Some(3000),
                    "node",
                    false,
                    "5",
                    false,
                    None,
                    Some("ready".to_string()),
                ),
                exit_code: 0,
            }),
        )
        .expect("json should render");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered.0).expect("json should parse"),
            serde_json::json!({
                "command": "ports logs 3000",
                "ok": true,
                "data": {
                    "pid": 42,
                    "port": 3000,
                    "process_name": "node",
                    "follow": false,
                    "lines": "5",
                    "stderr_only": false,
                    "source": null,
                    "output": "ready"
                },
                "error": null,
                "warnings": ["system command timed out; results may be incomplete"]
            })
        );
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn logs_json_error_includes_drained_user_warnings() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        record_user_warning(&PortError::Timeout {
            cmd: "lsof -p 42".to_string(),
            ms: 3000,
        });

        let rendered = render_logs_json_result(
            "ports logs abc",
            Err(LogsJsonError {
                code: "invalid_target".to_string(),
                message: "\"abc\" is not a valid port/PID".to_string(),
                exit_code: 1,
            }),
        )
        .expect("json should render");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered.0).expect("json should parse"),
            serde_json::json!({
                "command": "ports logs abc",
                "ok": false,
                "data": null,
                "error": {
                    "code": "invalid_target",
                    "message": "\"abc\" is not a valid port/PID"
                },
                "warnings": ["system command timed out; results may be incomplete"]
            })
        );
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn log_header_uses_ascii_safe_marker() {
        let _guard = crate::style::glyph_test_lock().lock().unwrap();
        crate::style::set_force_ascii(true);
        let header = log_header_lines(":3000", 42, "node");
        crate::style::set_force_ascii(false);

        assert!(
            header[0].contains("->"),
            "expected ascii-safe header marker: {}",
            header[0]
        );
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

#[allow(clippy::items_after_test_module)]
fn tail_file(path: &Path, lines: &str, follow: bool, grep: Option<&str>) -> i32 {
    if !follow {
        match read_log_output(path, lines, false) {
            Ok(output) => {
                let filtered = apply_grep_filter(output, grep);
                if !filtered.is_empty() {
                    println!("{filtered}");
                }
                return 0;
            }
            Err(_) => return 1,
        }
    }

    run_shell(&tail_follow_shell_command(path, lines, grep))
}

fn tail_follow_shell_command(path: &Path, lines: &str, grep: Option<&str>) -> String {
    let command = match build_tail_command(path, lines, true) {
        TailCommand::PowerShell { command } => format!("powershell -Command {command:?}"),
        TailCommand::Argv(argv) => shell_quote_argv(&argv),
    };
    apply_grep_to_shell_command(&command, grep)
}

fn shell_quote_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| {
            if arg
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-'))
            {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', r#"'\''"#))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_log_output(path: &Path, lines: &str, follow: bool) -> Result<String, String> {
    if follow {
        return Err("follow mode is not supported for collected log output".to_string());
    }
    let output = match build_tail_command(path, lines, false) {
        TailCommand::PowerShell { command } => Command::new("powershell")
            .args(["-Command", &command])
            .stdin(Stdio::null())
            .output(),
        TailCommand::Argv(argv) => {
            if argv.is_empty() {
                return Err("empty tail command".to_string());
            }
            let mut cmd = Command::new(&argv[0]);
            cmd.args(&argv[1..]).stdin(Stdio::null()).output()
        }
    }
    .map_err(|err| err.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

pub(crate) fn build_tail_command(
    path: &Path,
    lines: &str,
    follow: bool,
) -> crate::model::TailCommand {
    crate::platform::native_scanner().build_tail_command(path, lines, follow)
}

fn run_shell(cmd: &str) -> i32 {
    crate::platform::native_scanner().run_shell(cmd)
}

fn run_shell_output(cmd: &str) -> Result<String, String> {
    crate::platform::native_scanner().run_shell_output(cmd)
}
