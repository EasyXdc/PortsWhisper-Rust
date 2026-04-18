use crate::model::{KillResolutionKind, PortInfo};
use crate::scanner;
use crate::style;
use std::process::{Command, Stdio};

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
    } else {
        "-TERM"
    };
    ("kill".to_string(), vec![sig.to_string(), pid.to_string()])
}

pub fn run_kill(args: &[String]) -> i32 {
    run_kill_with(args, scanner::resolve_kill_target, kill_process)
}

fn run_kill_with<Resolve, Kill>(args: &[String], resolve: Resolve, kill: Kill) -> i32
where
    Resolve: Fn(u32) -> Option<crate::model::KillTargetResolution>,
    Kill: Fn(u32, &str) -> bool,
{
    let force = args.iter().any(|a| a == "-f" || a == "--force");
    let raw_targets: Vec<String> = args
        .iter()
        .filter(|a| a.as_str() != "-f" && a.as_str() != "--force")
        .cloned()
        .collect();
    if raw_targets.is_empty() {
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
        return 1;
    }
    let mut targets = Vec::new();
    let mut range_spans = Vec::new();
    for target in raw_targets {
        if let Some((start, end)) = parse_range(&target) {
            if let Err(message) = validate_range_target(&target, start, end) {
                println!("{}", style::red(format!("\n  ✕ {message}\n")));
                return 1;
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

    let signal = if force { "SIGKILL" } else { "SIGTERM" };
    let mut any_failed = false;
    let mut killed = 0;
    let mut empty = 0;
    println!();
    for (idx, target) in targets.iter().enumerate() {
        let from_range = range_spans.iter().any(|(s, e)| idx >= *s && idx < *e);
        let Ok(n) = target.parse::<u32>() else {
            println!(
                "{}",
                style::red(format!("  ✕ \"{target}\" is not a valid port/PID"))
            );
            any_failed = true;
            continue;
        };
        if n.to_string() != target.trim() {
            println!(
                "{}",
                style::red(format!("  ✕ \"{target}\" is not a valid port/PID"))
            );
            any_failed = true;
            continue;
        }
        let Some(resolved) = resolve(n) else {
            if from_range {
                empty += 1;
                continue;
            }
            let msg = if n <= 65_535 {
                format!("No listener on :{n} and no process with PID {n}")
            } else {
                format!("No process with PID {n}")
            };
            println!("{}", style::red(format!("  ✕ {msg}")));
            any_failed = true;
            continue;
        };
        let label = match resolved.via {
            KillResolutionKind::Port => {
                let port = resolved.port.unwrap_or(n as u16);
                let process = resolved
                    .info
                    .as_ref()
                    .map(process_name)
                    .unwrap_or_else(|| "unknown".to_string());
                format!(":{port} — {process} (PID {})", resolved.pid)
            }
            KillResolutionKind::Pid => format!("PID {}", resolved.pid),
        };
        println!("{}", style::white(format!("  Killing {label}")));
        if kill(resolved.pid, signal) {
            println!("{}", style::green(format!("  ✓ Sent {signal} to {label}")));
            killed += 1;
        } else {
            println!(
                "{}",
                style::red(format!(
                    "  ✕ Failed. Try: sudo kill{} {}",
                    if force { " -9" } else { "" },
                    resolved.pid
                ))
            );
            any_failed = true;
        }
    }
    if !range_spans.is_empty() {
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
    if any_failed { 1 } else { 0 }
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
    use super::{build_kill_command, parse_range, run_kill_with, validate_range_target};
    use crate::model::{KillResolutionKind, KillTargetResolution, PortInfo, ProcessStatus};
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
        let exit = run_kill_with(
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
        assert_eq!(exit, 0);
        assert_eq!(
            attempts.into_inner(),
            vec![(42, "SIGKILL".to_string()), (70000, "SIGKILL".to_string())]
        );
    }

    #[test]
    fn range_targets_expand_and_skip_empty_ports_without_failing() {
        let attempts = RefCell::new(Vec::new());
        let exit = run_kill_with(
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

        assert_eq!(exit, 0);
        assert_eq!(
            attempts.into_inner(),
            vec![(40, "SIGTERM".to_string()), (42, "SIGTERM".to_string())]
        );
    }

    #[test]
    fn exit_code_is_nonzero_when_any_target_fails() {
        let exit = run_kill_with(
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

        assert_eq!(exit, 1);
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
}
