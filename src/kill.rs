use crate::json_output;
use crate::model::{KillResolutionKind, PortInfo};
use crate::scanner;
use crate::style;
use std::process::{Command, Stdio};

#[derive(Debug, Eq, PartialEq)]
struct KillJsonResult {
    signal: String,
    targets: Vec<json_output::KillTargetPayload>,
    range_spans: Vec<(usize, usize)>,
    exit_code: i32,
}

pub fn kill_process(pid: u32, signal: &str) -> bool {
    let (program, args) = build_kill_command(pid, signal, cfg!(target_os = "windows"));
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn build_kill_command(pid: u32, signal: &str, windows_mode: bool) -> (String, Vec<String>) {
    if windows_mode {
        if signal == "SIGKILL" {
            return (
                "taskkill".to_string(),
                vec!["/F".to_string(), "/PID".to_string(), pid.to_string()],
            );
        }
        return (
            "taskkill".to_string(),
            vec!["/PID".to_string(), pid.to_string()],
        );
    }

    let sig = if signal == "SIGKILL" {
        "-KILL"
    } else if signal == "SIGINT" {
        "-INT"
    } else {
        "-TERM"
    };
    ("kill".to_string(), vec![sig.to_string(), pid.to_string()])
}

fn parse_requested_signal(args: &[String]) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--signal" {
            let value = args.get(index + 1)?;
            return normalize_signal_name(value);
        }
        index += 1;
    }
    None
}

fn normalize_signal_name(value: &str) -> Option<String> {
    let upper = value.trim().to_ascii_uppercase();
    match upper.as_str() {
        "TERM" | "SIGTERM" => Some("SIGTERM".to_string()),
        "KILL" | "SIGKILL" => Some("SIGKILL".to_string()),
        "INT" | "SIGINT" => Some("SIGINT".to_string()),
        _ => None,
    }
}

fn signal_flag_error(args: &[String]) -> Option<KillJsonResult> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--signal" {
            let Some(value) = args.get(index + 1) else {
                return Some(invalid_signal_result(String::new(), "missing value for --signal"));
            };
            if normalize_signal_name(value).is_none() {
                return Some(invalid_signal_result(
                    value.clone(),
                    format!(
                        "invalid signal '{value}' (supported: SIGTERM, SIGINT, SIGKILL)"
                    ),
                ));
            }
            return None;
        }
        index += 1;
    }
    None
}

fn invalid_signal_result(input: String, message: impl Into<String>) -> KillJsonResult {
    KillJsonResult {
        signal: "SIGTERM".to_string(),
        targets: vec![json_output::KillTargetPayload {
            input,
            pid: None,
            port: None,
            via: None,
            process_name: None,
            success: false,
            message: message.into(),
        }],
        range_spans: Vec::new(),
        exit_code: 1,
    }
}

fn signal_hint(signal: &str) -> &'static str {
    match signal {
        "SIGKILL" => " -9",
        "SIGINT" => " -INT",
        _ => "",
    }
}

pub fn run_kill(args: &[String]) -> i32 {
    let result = run_kill_json_with(args, scanner::resolve_kill_target, kill_process);
    print_kill_result(&result);
    result.exit_code
}

pub fn run_kill_json(args: &[String]) -> i32 {
    let result = run_kill_json_with(args, scanner::resolve_kill_target, kill_process);
    let exit_code = result.exit_code;
    match json_output::render_json(&kill_json_envelope(args, result)) {
        Ok(output) => {
            println!("{output}");
            exit_code
        }
        Err(err) => {
            eprintln!("failed to render json for ports kill: {err}");
            1
        }
    }
}

fn command_string(args: &[String]) -> String {
    if args.is_empty() {
        "ports kill".to_string()
    } else {
        format!("ports kill {}", args.join(" "))
    }
}

fn kill_json_envelope(
    args: &[String],
    result: KillJsonResult,
) -> json_output::CommandEnvelope<json_output::KillPayload> {
    let command = command_string(args);
    if let Some((code, message)) = command_error(&result) {
        json_output::CommandEnvelope::err(command, code, message)
    } else {
        json_output::CommandEnvelope::ok(
            command,
            json_output::kill_payload(result.signal, result.targets),
        )
    }
}

fn command_error(result: &KillJsonResult) -> Option<(&'static str, String)> {
    let [target] = result.targets.as_slice() else {
        return None;
    };
    if target.success || target.pid.is_some() || target.port.is_some() || target.via.is_some() {
        return None;
    }
    if target.message.starts_with("Usage: ports kill") {
        return Some(("usage", target.message.clone()));
    }
    if target.message.starts_with("Invalid range:")
        || target.message.starts_with("Range too large:")
    {
        return Some(("invalid_target", target.message.clone()));
    }
    if target.message.contains("not a valid port/PID") {
        return Some(("invalid_target", target.message.clone()));
    }
    if target.message.starts_with("No listener on :")
        || target.message.starts_with("No process with PID ")
    {
        return Some(("target_not_found", target.message.clone()));
    }
    None
}

fn run_kill_json_with<Resolve, Kill>(
    args: &[String],
    resolve: Resolve,
    kill: Kill,
) -> KillJsonResult
where
    Resolve: Fn(u32) -> Option<crate::model::KillTargetResolution>,
    Kill: Fn(u32, &str) -> bool,
{
    let force = args.iter().any(|a| a == "-f" || a == "--force");
    if !force {
        if let Some(result) = signal_flag_error(args) {
            return result;
        }
    }
    let raw_targets: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|(idx, arg)| {
            let is_force = arg.as_str() == "-f" || arg.as_str() == "--force";
            let is_signal_flag = arg.as_str() == "--signal";
            let is_signal_value = *idx > 0 && args[*idx - 1].as_str() == "--signal";
            !is_force && !is_signal_flag && !is_signal_value
        })
        .map(|(_, arg)| arg.clone())
        .collect();
    let signal = if force {
        "SIGKILL".to_string()
    } else {
        parse_requested_signal(args).unwrap_or_else(|| "SIGTERM".to_string())
    };

    if raw_targets.is_empty() {
        return KillJsonResult {
            signal: signal.clone(),
            targets: vec![json_output::KillTargetPayload {
                input: String::new(),
                pid: None,
                port: None,
                via: None,
                process_name: None,
                success: false,
                message: "Usage: ports kill [-f|--force] <port|pid|range> [port|pid|range...]"
                    .to_string(),
            }],
            range_spans: Vec::new(),
            exit_code: 1,
        };
    }

    let mut targets = Vec::new();
    let mut range_spans = Vec::new();
    for target in raw_targets {
        if let Some((start, end)) = parse_range(&target) {
            if let Err(message) = validate_range_target(&target, start, end) {
                return KillJsonResult {
                    signal: signal.clone(),
                    targets: vec![json_output::KillTargetPayload {
                        input: target,
                        pid: None,
                        port: None,
                        via: None,
                        process_name: None,
                        success: false,
                        message,
                    }],
                    range_spans: Vec::new(),
                    exit_code: 1,
                };
            }
            let start_idx = targets.len();
            for port in start..=end {
                targets.push(port.to_string());
            }
            range_spans.push((start_idx, targets.len()));
        } else {
            targets.push(target);
        }
    }

    let mut any_failed = false;
    let mut results = Vec::with_capacity(targets.len());
    for (idx, target) in targets.iter().enumerate() {
        let from_range = range_spans.iter().any(|(s, e)| idx >= *s && idx < *e);
        let Ok(n) = target.parse::<u32>() else {
            any_failed = true;
            results.push(json_output::KillTargetPayload {
                input: target.clone(),
                pid: None,
                port: None,
                via: None,
                process_name: None,
                success: false,
                message: format!("\"{target}\" is not a valid port/PID"),
            });
            continue;
        };
        if n.to_string() != target.trim() {
            any_failed = true;
            results.push(json_output::KillTargetPayload {
                input: target.clone(),
                pid: None,
                port: None,
                via: None,
                process_name: None,
                success: false,
                message: format!("\"{target}\" is not a valid port/PID"),
            });
            continue;
        }
        let Some(resolved) = resolve(n) else {
            let msg = if n <= 65_535 {
                format!("No listener on :{n} and no process with PID {n}")
            } else {
                format!("No process with PID {n}")
            };
            if !from_range {
                any_failed = true;
            }
            results.push(json_output::KillTargetPayload {
                input: target.clone(),
                pid: None,
                port: None,
                via: None,
                process_name: None,
                success: false,
                message: msg,
            });
            continue;
        };

        let port = resolved.port.or(match resolved.via {
            KillResolutionKind::Port => Some(n as u16),
            KillResolutionKind::Pid => None,
        });
        let process_name = resolved.info.as_ref().map(process_name);
        let label = match resolved.via {
            KillResolutionKind::Port => {
                let port = port.unwrap_or(n as u16);
                let process = process_name
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                format!(":{port} — {process} (PID {})", resolved.pid)
            }
            KillResolutionKind::Pid => format!("PID {}", resolved.pid),
        };
        let success = kill(resolved.pid, &signal);
        if !success {
            any_failed = true;
        }
        results.push(json_output::KillTargetPayload {
            input: target.clone(),
            pid: Some(resolved.pid),
            port,
            via: Some(match resolved.via {
                KillResolutionKind::Port => "port".to_string(),
                KillResolutionKind::Pid => "pid".to_string(),
            }),
            process_name,
            success,
            message: if success {
                format!("Sent {signal} to {label}")
            } else {
                format!(
                    "Failed. Try: sudo kill{} {}",
                    signal_hint(&signal),
                    resolved.pid
                )
            },
        });
    }

    KillJsonResult {
        signal,
        targets: results,
        range_spans,
        exit_code: if any_failed { 1 } else { 0 },
    }
}

fn print_kill_result(result: &KillJsonResult) {
    if result.targets.len() == 1 && result.targets[0].input.is_empty() {
        println!(
            "{}",
            style::red("\n  Usage: ports kill [-f|--force] <port|pid|range> [port|pid|range...]\n")
        );
        println!(
            "{}",
            style::gray(
                "  Kills listener on port (1-65535), or process by PID. Use -f for SIGKILL."
            )
        );
        println!("{}", style::gray("  Ranges: ports kill 3000-3010\n"));
        return;
    }

    println!();
    let mut killed = 0usize;
    let mut empty = 0usize;
    let mut any_failed = false;
    for idx in 0..result.targets.len() {
        let target = &result.targets[idx];
        let from_range = result.range_spans.iter().any(|(start, end)| idx >= *start && idx < *end);
        if from_range && !target.success && target.pid.is_none() && target.message.starts_with("No listener on :") {
            empty += 1;
            continue;
        }
        if let Some(line) = render_kill_target_line(result, idx) {
            println!("{line}");
        }
        match (target.success, target.pid) {
            (true, Some(_)) => killed += 1,
            (false, Some(_)) | (false, None) => any_failed = true,
            (true, None) => {}
        }
    }
    if !result.range_spans.is_empty() {
        let mut parts = Vec::new();
        if killed > 0 {
            parts.push(style::green(format!("{killed} killed")));
        }
        if empty > 0 {
            parts.push(style::gray(format!("{empty} empty")));
        }
        if any_failed {
            parts.push(style::red("some failed"));
        }
        println!(
            "  {} {}",
            style::dim("Range summary:"),
            parts.join(&style::dim(", "))
        );
    }
    println!();
}

fn render_kill_target_line(result: &KillJsonResult, index: usize) -> Option<String> {
    let target = result.targets.get(index)?;
    let glyphs = style::glyphs();
    let mut out = String::new();
    match (target.success, target.pid) {
        (true, Some(_)) => {
            let prefix = if let Some(label) = kill_target_label(target) {
                style::white(format!("  Killing {label}"))
            } else {
                style::white("  Killing target")
            };
            out.push_str(&prefix);
            out.push('\n');
            out.push_str(&style::green(format!("  {} {}", glyphs.success, target.message)));
            Some(out)
        }
        (false, Some(_)) => {
            if let Some(label) = kill_target_label(target) {
                out.push_str(&style::white(format!("  Killing {label}")));
                out.push('\n');
            }
            out.push_str(&style::red(format!("  {} {}", glyphs.failure, target.message)));
            Some(out)
        }
        (false, None) => Some(style::red(format!("  {} {}", glyphs.failure, target.message))),
        (true, None) => None,
    }
}

fn kill_target_label(target: &json_output::KillTargetPayload) -> Option<String> {
    match target.via.as_deref() {
        Some("port") => Some(format!(
            ":{} — {} (PID {})",
            target.port?,
            target
                .process_name
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            target.pid?
        )),
        Some("pid") => Some(format!("PID {}", target.pid?)),
        _ => None,
    }
}

fn process_name(info: &PortInfo) -> String {
    if info.process_name.is_empty() {
        info.raw_name.clone()
    } else {
        info.process_name.clone()
    }
}

fn parse_range(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once('-')?;
    if a.is_empty() || b.is_empty() || a.contains('-') || b.contains('-') {
        return None;
    }
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn validate_range_target(target: &str, start: u32, end: u32) -> Result<(), String> {
    if start > end {
        return Err(format!(
            "Invalid range: {target} (start must be less than end)"
        ));
    }
    if end - start > 1000 {
        return Err(format!("Range too large: {target} (max 1000 ports)"));
    }
    if start < 1 || end > 65_535 {
        return Err(format!("Invalid range: {target} (ports must be 1-65535)"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        KillJsonResult, build_kill_command, command_string, kill_json_envelope, parse_range,
        render_kill_target_line, run_kill_json_with, validate_range_target,
    };
    use crate::json_output;
    use crate::model::{KillResolutionKind, KillTargetResolution, PortInfo, ProcessStatus};
    use serde_json::json;
    use std::cell::RefCell;

    #[test]
    fn parse_range_accepts_numeric_ranges_only() {
        assert_eq!(parse_range("3000-3010"), Some((3000, 3010)));
        assert_eq!(parse_range("3000"), None);
        assert_eq!(parse_range("3000-abc"), None);
        assert_eq!(parse_range("3000-3010-3020"), None);
    }

    #[test]
    fn invalid_ranges_report_reference_boundaries() {
        assert!(
            validate_range_target("3010-3000", 3010, 3000)
                .unwrap_err()
                .contains("start must be less than end")
        );
        assert!(
            validate_range_target("1-1002", 1, 1002)
                .unwrap_err()
                .contains("max 1000 ports")
        );
        assert!(
            validate_range_target("0-10", 0, 10)
                .unwrap_err()
                .contains("ports must be 1-65535")
        );
        assert!(
            validate_range_target("65535-65536", 65535, 65536)
                .unwrap_err()
                .contains("ports must be 1-65535")
        );
        assert!(validate_range_target("3000-3010", 3000, 3010).is_ok());
    }

    #[test]
    fn injected_killer_verifies_kill_without_sending_real_signal() {
        let attempts = RefCell::new(Vec::new());
        let args = vec![
            "--force".to_string(),
            "3000".to_string(),
            "70000".to_string(),
        ];
        let result = run_kill_json_with(
            &args,
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 42,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 42)),
                }),
                70000 => Some(KillTargetResolution {
                    pid: 70000,
                    via: KillResolutionKind::Pid,
                    port: None,
                    info: None,
                }),
                _ => None,
            },
            |pid, signal| {
                attempts.borrow_mut().push((pid, signal.to_string()));
                true
            },
        );
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            attempts.into_inner(),
            vec![(42, "SIGKILL".to_string()), (70000, "SIGKILL".to_string())]
        );
    }

    #[test]
    fn explicit_signal_selection_is_used_for_targeted_kills() {
        let attempts = RefCell::new(Vec::new());
        let args = vec![
            "--signal".to_string(),
            "sigint".to_string(),
            "3000".to_string(),
        ];
        let result = run_kill_json_with(
            &args,
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 42,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 42)),
                }),
                _ => None,
            },
            |pid, signal| {
                attempts.borrow_mut().push((pid, signal.to_string()));
                true
            },
        );

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.signal, "SIGINT");
        assert_eq!(
            attempts.into_inner(),
            vec![(42, "SIGINT".to_string())]
        );
    }

    #[test]
    fn invalid_signal_selection_fails_instead_of_falling_back() {
        let args = vec![
            "--signal".to_string(),
            "sigusr1".to_string(),
            "3000".to_string(),
        ];

        let result = run_kill_json_with(&args, |_| None, |_pid, _signal| true);

        assert_eq!(result.exit_code, 1);
        assert_eq!(result.targets.len(), 1);
        assert_eq!(result.targets[0].input, "sigusr1");
        assert!(result.targets[0].message.contains("invalid signal"));
    }

    #[test]
    fn failed_kill_guidance_preserves_selected_signal() {
        let result = run_kill_json_with(
            &[
                "--signal".to_string(),
                "sigint".to_string(),
                "3000".to_string(),
            ],
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 42,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 42)),
                }),
                _ => None,
            },
            |_pid, _signal| false,
        );

        assert_eq!(result.exit_code, 1);
        assert_eq!(result.signal, "SIGINT");
        assert_eq!(result.targets.len(), 1);
        assert_eq!(result.targets[0].pid, Some(42));
        assert!(result.targets[0].message.contains("sudo kill -INT 42"));
    }

    #[test]
    fn range_targets_expand_and_skip_empty_ports_without_failing() {
        let attempts = RefCell::new(Vec::new());
        let result = run_kill_json_with(
            &["3000-3002".to_string()],
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 40,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 40)),
                }),
                3002 => Some(KillTargetResolution {
                    pid: 42,
                    via: KillResolutionKind::Port,
                    port: Some(3002),
                    info: Some(fake_port(3002, 42)),
                }),
                _ => None,
            },
            |pid, signal| {
                attempts.borrow_mut().push((pid, signal.to_string()));
                true
            },
        );

        assert_eq!(result.exit_code, 0);
        assert_eq!(
            attempts.into_inner(),
            vec![(40, "SIGTERM".to_string()), (42, "SIGTERM".to_string())]
        );
    }

    #[test]
    fn exit_code_is_nonzero_when_any_target_fails() {
        let result = run_kill_json_with(
            &["3000".to_string(), "3001".to_string()],
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 40,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 40)),
                }),
                3001 => Some(KillTargetResolution {
                    pid: 41,
                    via: KillResolutionKind::Port,
                    port: Some(3001),
                    info: Some(fake_port(3001, 41)),
                }),
                _ => None,
            },
            |pid, _| pid == 40,
        );

        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn json_kill_result_includes_per_target_status_and_resolution() {
        let attempts = RefCell::new(Vec::new());
        let result = run_kill_json_with(
            &["3000".to_string(), "3001".to_string(), "abc".to_string()],
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 40,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 40)),
                }),
                3001 => Some(KillTargetResolution {
                    pid: 41,
                    via: KillResolutionKind::Pid,
                    port: None,
                    info: None,
                }),
                _ => None,
            },
            |pid, signal| {
                attempts.borrow_mut().push((pid, signal.to_string()));
                pid == 40
            },
        );

        assert_eq!(result.exit_code, 1);
        assert_eq!(result.signal, "SIGTERM");
        assert_eq!(
            attempts.into_inner(),
            vec![(40, "SIGTERM".to_string()), (41, "SIGTERM".to_string())]
        );
        assert_eq!(result.targets.len(), 3);
        assert_eq!(result.targets[0].input, "3000");
        assert_eq!(result.targets[0].pid, Some(40));
        assert_eq!(result.targets[0].port, Some(3000));
        assert_eq!(result.targets[0].via.as_deref(), Some("port"));
        assert_eq!(result.targets[0].process_name.as_deref(), Some("node"));
        assert!(result.targets[0].success);
        assert!(result.targets[0].message.contains("Sent SIGTERM"));

        assert_eq!(result.targets[1].input, "3001");
        assert_eq!(result.targets[1].pid, Some(41));
        assert_eq!(result.targets[1].port, None);
        assert_eq!(result.targets[1].via.as_deref(), Some("pid"));
        assert_eq!(result.targets[1].process_name, None);
        assert!(!result.targets[1].success);
        assert!(result.targets[1].message.contains("sudo kill"));

        assert_eq!(result.targets[2].input, "abc");
        assert_eq!(result.targets[2].pid, None);
        assert!(!result.targets[2].success);
        assert!(result.targets[2].message.contains("not a valid port/PID"));
    }

    #[test]
    fn kill_json_command_string_includes_subcommand_name() {
        assert_eq!(command_string(&[]), "ports kill");
        assert_eq!(command_string(&["3000".to_string()]), "ports kill 3000");
    }

    #[test]
    fn kill_json_uses_error_envelope_for_usage_failures() {
        let result = run_kill_json_with(&[], |_| None, |_pid, _signal| true);

        let rendered = json_output::render_json(&kill_json_envelope(&[], result))
            .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports kill",
                "ok": false,
                "data": null,
                "error": {
                    "code": "usage",
                    "message": "Usage: ports kill [-f|--force] <port|pid|range> [port|pid|range...]"
                }
            })
        );
    }

    #[test]
    fn kill_json_uses_error_envelope_for_invalid_ranges() {
        let result = run_kill_json_with(&["3010-3000".to_string()], |_| None, |_pid, _signal| true);

        let rendered =
            json_output::render_json(&kill_json_envelope(&["3010-3000".to_string()], result))
                .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports kill 3010-3000",
                "ok": false,
                "data": null,
                "error": {
                    "code": "invalid_target",
                    "message": "Invalid range: 3010-3000 (start must be less than end)"
                }
            })
        );
    }

    #[test]
    fn kill_json_uses_error_envelope_for_single_invalid_target() {
        let result = run_kill_json_with(&["abc".to_string()], |_| None, |_pid, _signal| true);

        let rendered = json_output::render_json(&kill_json_envelope(&["abc".to_string()], result))
            .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports kill abc",
                "ok": false,
                "data": null,
                "error": {
                    "code": "invalid_target",
                    "message": "\"abc\" is not a valid port/PID"
                }
            })
        );
    }

    #[test]
    fn kill_json_uses_error_envelope_for_single_unresolved_target() {
        let result = run_kill_json_with(&["3000".to_string()], |_| None, |_pid, _signal| true);

        let rendered = json_output::render_json(&kill_json_envelope(&["3000".to_string()], result))
            .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports kill 3000",
                "ok": false,
                "data": null,
                "error": {
                    "code": "target_not_found",
                    "message": "No listener on :3000 and no process with PID 3000"
                }
            })
        );
    }

    #[test]
    fn range_json_result_still_tracks_empty_entries_for_text_summary() {
        let result = run_kill_json_with(
            &["3000-3002".to_string()],
            |target| match target {
                3000 => Some(KillTargetResolution {
                    pid: 40,
                    via: KillResolutionKind::Port,
                    port: Some(3000),
                    info: Some(fake_port(3000, 40)),
                }),
                3002 => Some(KillTargetResolution {
                    pid: 42,
                    via: KillResolutionKind::Port,
                    port: Some(3002),
                    info: Some(fake_port(3002, 42)),
                }),
                _ => None,
            },
            |_pid, _signal| true,
        );

        assert_eq!(result.targets.len(), 3);
        assert_eq!(result.targets[1].input, "3001");
        assert!(result.targets[1].message.contains("No listener on :3001"));
    }

    #[test]
    fn windows_kill_command_uses_taskkill_for_term_and_force() {
        assert_eq!(
            build_kill_command(42, "SIGTERM", true),
            (
                "taskkill".to_string(),
                vec!["/PID".to_string(), "42".to_string()]
            )
        );
        assert_eq!(
            build_kill_command(42, "SIGKILL", true),
            (
                "taskkill".to_string(),
                vec!["/F".to_string(), "/PID".to_string(), "42".to_string()]
            )
        );
    }

    #[test]
    fn ascii_kill_result_uses_ascii_success_and_failure_markers() {
        let _guard = crate::style::glyph_test_lock().lock().unwrap();
        crate::style::set_force_ascii(true);

        let success = strip_ansi(render_kill_target_line(&KillJsonResult {
            signal: "SIGTERM".to_string(),
            targets: vec![json_output::KillTargetPayload {
                input: "3000".to_string(),
                pid: Some(42),
                port: Some(3000),
                via: Some("port".to_string()),
                process_name: Some("node".to_string()),
                success: true,
                message: "Sent SIGTERM to :3000 — node (PID 42)".to_string(),
            }],
            range_spans: Vec::new(),
            exit_code: 0,
        }, 0).expect("success line should render"));

        let failure = strip_ansi(render_kill_target_line(&KillJsonResult {
            signal: "SIGTERM".to_string(),
            targets: vec![json_output::KillTargetPayload {
                input: "3000".to_string(),
                pid: Some(42),
                port: Some(3000),
                via: Some("port".to_string()),
                process_name: Some("node".to_string()),
                success: false,
                message: "Failed. Try: sudo kill -9 42".to_string(),
            }],
            range_spans: Vec::new(),
            exit_code: 1,
        }, 0).expect("failure line should render"));

        crate::style::set_force_ascii(false);

        assert!(success.contains("  v Sent SIGTERM"), "expected ascii success marker: {success}");
        assert!(failure.contains("  x Failed."), "expected ascii failure marker: {failure}");
    }

    fn fake_port(port: u16, pid: u32) -> PortInfo {
        PortInfo {
            port,
            pid,
            process_name: "node".to_string(),
            raw_name: "node".to_string(),
            command: "node server.js".to_string(),
            cwd: None,
            project_name: None,
            framework: None,
            uptime: None,
            start_time: None,
            status: ProcessStatus::Healthy,
            memory: None,
            git_branch: None,
            process_tree: Vec::new(),
        }
    }

    fn strip_ansi(s: String) -> String {
        let mut out = String::new();
        let mut esc = false;
        for ch in s.chars() {
            if esc {
                if ch == 'm' {
                    esc = false;
                }
                continue;
            }
            if ch == '\x1b' {
                esc = true;
                continue;
            }
            out.push(ch);
        }
        out
    }
}
