use crate::display;
use crate::error;
use crate::json_output;
use crate::kill;
use crate::logs;
use crate::ports;
use crate::scanner;
use crate::style;
use crate::util::prompt_line;
use crate::watch;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedCli {
    pub show_all: bool,
    pub verbose: bool,
    pub json: bool,
    pub command: CliCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CliCommand {
    List,
    Help,
    Ps,
    Clean,
    Kill(Vec<String>),
    Logs(Vec<String>),
    Watch,
    PortDetail(u32),
    Unknown(String),
}

pub fn run(_binary_name: &str, args: Vec<String>) -> i32 {
    let parsed = parse_args(&args);
    error::set_verbose_enabled(parsed.verbose);
    error::drain_user_warnings();
    let exit_code = dispatch(parsed);
    if error::verbose_enabled() {
        let entries = error::drain_verbose_log();
        for line in format_verbose_entries(&entries) {
            eprintln!("{line}");
        }
        error::set_verbose_enabled(false);
    }
    exit_code
}

fn dispatch(parsed: ParsedCli) -> i32 {
    if parsed.json && !supports_json(&parsed.command) {
        eprintln!("{}", json_not_supported_message(&parsed.command));
        return 1;
    }

    match parsed.command {
        CliCommand::List => {
            let mut ports = ports::get_listening_ports(false);
            if !parsed.show_all {
                ports.retain(|p| scanner::is_dev_process(&p.process_name, &p.command));
            }
            if parsed.json {
                return match render_list_json(&ports) {
                    Ok(output) => {
                        println!("{output}");
                        0
                    }
                    Err(err) => {
                        eprintln!("failed to render json for ports: {err}");
                        1
                    }
                };
            }
            display::display_port_table(&ports, !parsed.show_all);
            print_warning_lines(&error::drain_user_warnings());
            0
        }
        CliCommand::Help => {
            if parsed.json {
                return match format_help_json() {
                    Ok(output) => {
                        println!("{output}");
                        0
                    }
                    Err(err) => {
                        eprintln!("failed to render json for ports help: {err}");
                        1
                    }
                };
            }
            print_help();
            0
        }
        CliCommand::Ps => {
            let mut processes = if parsed.show_all {
                scanner::get_all_processes()
            } else {
                scanner::get_all_dev_processes()
            };
            if !parsed.show_all {
                let docker: Vec<_> = processes
                    .iter()
                    .filter(|p| scanner::is_docker_process(&p.process_name))
                    .cloned()
                    .collect();
                let mut non_docker: Vec<_> = processes
                    .into_iter()
                    .filter(|p| !scanner::is_docker_process(&p.process_name))
                    .collect();
                if !docker.is_empty() {
                    let total_cpu: f32 = docker.iter().map(|p| p.cpu).sum();
                    let total_rss: u64 = docker.iter().map(|p| p.rss_kb).sum();
                    let first = &docker[0];
                    non_docker.push(crate::model::ProcessInfo {
                        pid: first.pid,
                        ppid: first.ppid,
                        process_name: "Docker".to_string(),
                        command: String::new(),
                        description: format!("{} processes", docker.len()),
                        cpu: total_cpu,
                        rss_kb: total_rss,
                        memory: Some(crate::util::format_memory(total_rss)),
                        cwd: None,
                        project_name: None,
                        framework: Some("Docker".to_string()),
                        uptime: first.uptime.clone(),
                        status_raw: String::new(),
                    });
                }
                processes = non_docker;
            }
            processes.sort_by(|a, b| {
                b.cpu
                    .partial_cmp(&a.cpu)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            if parsed.json {
                return match render_ps_json(&processes) {
                    Ok(output) => {
                        println!("{output}");
                        0
                    }
                    Err(err) => {
                        eprintln!("failed to render json for ports ps: {err}");
                        1
                    }
                };
            }
            display::display_process_table(&processes, !parsed.show_all);
            print_warning_lines(&error::drain_user_warnings());
            0
        }
        CliCommand::Clean => {
            if parsed.json {
                return run_clean_json();
            }
            run_clean()
        }
        CliCommand::Kill(args) => {
            if parsed.json {
                return kill::run_kill_json(&args);
            }
            kill::run_kill(&args)
        }
        CliCommand::Logs(args) => {
            if parsed.json {
                return logs::run_logs_json(&args);
            }
            logs::run_logs(&args)
        }
        CliCommand::Watch => {
            if parsed.json {
                return watch::run_watch_json();
            }
            watch::run_watch()
        }
        CliCommand::PortDetail(port) => run_port_detail(port, parsed.json),
        CliCommand::Unknown(other) => {
            if parsed.json {
                return match format_unknown_command_json(&other) {
                    Ok(output) => {
                        println!("{output}");
                        1
                    }
                    Err(err) => {
                        eprintln!("failed to render json for ports {other}: {err}");
                        1
                    }
                };
            }
            for line in unknown_command_lines(&other) {
                println!("{line}");
            }
            1
        }
    }
}

fn supports_json(command: &CliCommand) -> bool {
    matches!(
        command,
        CliCommand::List
            | CliCommand::Help
            | CliCommand::Ps
            | CliCommand::Clean
            | CliCommand::Kill(_)
            | CliCommand::Logs(_)
            | CliCommand::Watch
            | CliCommand::PortDetail(_)
            | CliCommand::Unknown(_)
    )
}

fn json_not_supported_message(command: &CliCommand) -> String {
    format!(
        "JSON output is not supported for '{}' yet.",
        command_name(command)
    )
}

fn command_name(command: &CliCommand) -> &'static str {
    match command {
        CliCommand::List => "ports",
        CliCommand::Help => "help",
        CliCommand::Ps => "ps",
        CliCommand::Clean => "clean",
        CliCommand::Kill(_) => "kill",
        CliCommand::Logs(_) => "logs",
        CliCommand::Watch => "watch",
        CliCommand::PortDetail(_) => "detail",
        CliCommand::Unknown(_) => "unknown",
    }
}

pub fn parse_args(args: &[String]) -> ParsedCli {
    let show_all = args.iter().any(|a| a == "--all" || a == "-a");
    let verbose = args.iter().any(|a| a == "--verbose");
    let json = args.iter().any(|a| a == "--json");
    let filtered_args: Vec<String> = args
        .iter()
        .filter(|a| {
            let s = a.as_str();
            s != "--all" && s != "-a" && s != "--verbose" && s != "--json"
        })
        .cloned()
        .collect();
    let command = match filtered_args.first().map(String::as_str) {
        None => CliCommand::List,
        Some("help" | "--help" | "-h") => CliCommand::Help,
        Some("ps") => CliCommand::Ps,
        Some("clean") => CliCommand::Clean,
        Some("kill") => CliCommand::Kill(filtered_args[1..].to_vec()),
        Some("logs") => CliCommand::Logs(filtered_args),
        Some("watch") => CliCommand::Watch,
        Some(other) => other
            .parse::<u32>()
            .map(CliCommand::PortDetail)
            .unwrap_or_else(|_| CliCommand::Unknown(other.to_string())),
    };
    ParsedCli {
        show_all,
        verbose,
        json,
        command,
    }
}

fn run_port_detail(port: u32, json: bool) -> i32 {
    let info = if port <= u16::MAX as u32 {
        ports::get_port_details(port as u16)
    } else {
        None
    };
    if json {
        return match render_port_detail_json(port, info.as_ref()) {
            Ok(output) => {
                println!("{output}");
                0
            }
            Err(err) => {
                eprintln!("failed to render json for ports {port}: {err}");
                1
            }
        };
    }
    display::display_port_detail(info.as_ref());
    print_warning_lines(&error::drain_user_warnings());
    if let Some(info) = info {
        let prompt = detail_kill_prompt(port);
        if let Some(answer) = prompt_line(&prompt)
            && answer.to_lowercase() == "y"
        {
            if kill::kill_process(info.pid, "SIGTERM") {
                println!(
                    "{}",
                    style::green(format!("\n  ✓ Killed PID {}\n", info.pid))
                );
            } else {
                println!(
                    "{}",
                    style::red(format!("\n  ✕ Failed. Try: sudo kill -9 {}\n", info.pid))
                );
                return 1;
            }
        }
    }
    0
}

fn render_port_detail_json(
    port: u32,
    info: Option<&crate::model::PortInfo>,
) -> serde_json::Result<String> {
    render_query_json(format!("ports {port}"), json_output::detail_payload(info))
}

fn render_list_json(ports: &[crate::model::PortInfo]) -> serde_json::Result<String> {
    render_query_json("ports", json_output::list_payload(ports))
}

fn render_ps_json(processes: &[crate::model::ProcessInfo]) -> serde_json::Result<String> {
    render_query_json("ports ps", json_output::process_list_payload(processes))
}

fn render_query_json<T>(command: impl Into<String>, data: T) -> serde_json::Result<String>
where
    T: serde::Serialize,
{
    let warnings = drained_warning_messages();
    json_output::render_json(
        &json_output::CommandEnvelope::ok(command, data).with_warnings(warnings),
    )
}

fn format_warning_lines(warnings: &[error::PortError]) -> Vec<String> {
    warnings
        .iter()
        .map(|warning| format!("  warning: {}", warning.user_message()))
        .collect()
}

fn print_warning_lines(warnings: &[error::PortError]) {
    for line in format_warning_lines(warnings) {
        println!("{line}");
    }
}

fn drained_warning_messages() -> Vec<String> {
    error::drain_user_warnings()
        .into_iter()
        .map(|warning| warning.user_message())
        .collect()
}

fn format_verbose_entries(entries: &[String]) -> Vec<String> {
    if entries.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::with_capacity(entries.len() + 2);
    lines.push(String::new());
    lines.push(format!("  verbose log ({} entries):", entries.len()));
    lines.extend(entries.iter().map(|entry| format!("    {entry}")));
    lines
}

#[cfg(test)]
mod tests {
    use super::{
        CliCommand, ParsedCli, apply_clean_answer, clean_confirmation_prompt, detail_kill_prompt,
        dispatch, format_help_json, format_unknown_command_json, format_verbose_entries,
        format_warning_lines, json_not_supported_message, parse_args, render_list_json,
        render_port_detail_json, render_ps_json, run_clean_json_with, run_clean_with,
        supports_json, unknown_command_lines,
    };
    use crate::error::{PortError, drain_user_warnings, record_user_warning, verbose_test_lock};
    use crate::model::{PortInfo, ProcessStatus};
    use serde_json::json;
    use std::cell::RefCell;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn parses_public_commands_without_scanning() {
        assert_eq!(
            parse_args(&args(&[])),
            ParsedCli {
                show_all: false,
                verbose: false,
                json: false,
                command: CliCommand::List
            }
        );
        assert_eq!(parse_args(&args(&["help"])).command, CliCommand::Help);
        assert_eq!(parse_args(&args(&["--help"])).command, CliCommand::Help);
        assert_eq!(parse_args(&args(&["-h"])).command, CliCommand::Help);
        assert_eq!(parse_args(&args(&["ps"])).command, CliCommand::Ps);
        assert_eq!(parse_args(&args(&["clean"])).command, CliCommand::Clean);
        assert_eq!(parse_args(&args(&["watch"])).command, CliCommand::Watch);
        assert_eq!(
            parse_args(&args(&["3000"])).command,
            CliCommand::PortDetail(3000)
        );
    }

    #[test]
    fn normalizes_all_flags_globally_before_command_dispatch() {
        assert_eq!(
            parse_args(&args(&["--all"])),
            ParsedCli {
                show_all: true,
                verbose: false,
                json: false,
                command: CliCommand::List
            }
        );
        assert_eq!(
            parse_args(&args(&["ps", "-a"])),
            ParsedCli {
                show_all: true,
                verbose: false,
                json: false,
                command: CliCommand::Ps
            }
        );
        assert_eq!(
            parse_args(&args(&["kill", "--all", "3000"])),
            ParsedCli {
                show_all: true,
                verbose: false,
                json: false,
                command: CliCommand::Kill(args(&["3000"]))
            }
        );
    }

    #[test]
    fn recognizes_verbose_flag_globally_and_filters_it_out() {
        let parsed = parse_args(&args(&["--verbose"]));
        assert!(parsed.verbose);
        assert!(!parsed.json);
        assert_eq!(parsed.command, CliCommand::List);

        let parsed = parse_args(&args(&["ps", "--verbose", "--all"]));
        assert!(parsed.verbose);
        assert!(parsed.show_all);
        assert_eq!(parsed.command, CliCommand::Ps);

        let parsed = parse_args(&args(&["kill", "--verbose", "3000"]));
        assert!(parsed.verbose);
        assert_eq!(parsed.command, CliCommand::Kill(args(&["3000"])));

        assert!(!parse_args(&args(&["ps"])).verbose);
    }

    #[test]
    fn recognizes_json_flag_globally_and_filters_it_out() {
        let parsed = parse_args(&args(&["--json"]));
        assert!(parsed.json);
        assert_eq!(parsed.command, CliCommand::List);

        let parsed = parse_args(&args(&["ps", "--json", "--all"]));
        assert!(parsed.json);
        assert!(parsed.show_all);
        assert_eq!(parsed.command, CliCommand::Ps);

        let parsed = parse_args(&args(&["kill", "--json", "3000"]));
        assert!(parsed.json);
        assert_eq!(parsed.command, CliCommand::Kill(args(&["3000"])));

        assert!(!parse_args(&args(&["ps"])).json);
    }

    #[test]
    fn keeps_command_specific_arguments_after_normalization() {
        assert_eq!(
            parse_args(&args(&["kill", "-f", "3000", "3001"])).command,
            CliCommand::Kill(args(&["-f", "3000", "3001"]))
        );
        assert_eq!(
            parse_args(&args(&["logs", "3000", "--lines=5", "-f"])).command,
            CliCommand::Logs(args(&["logs", "3000", "--lines=5", "-f"]))
        );
        assert_eq!(
            parse_args(&args(&["unknown"])).command,
            CliCommand::Unknown("unknown".to_string())
        );
    }

    #[test]
    fn clean_prompt_flow_only_kills_on_yes_answer() {
        let orphaned = vec![fake_port(3000, 42), fake_port(3001, 43)];
        let attempts = RefCell::new(Vec::new());
        let outcome = apply_clean_answer(&orphaned, Some("n"), |pid, signal| {
            attempts.borrow_mut().push((pid, signal.to_string()));
            true
        });
        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.killed.is_empty());
        assert!(outcome.failed.is_empty());
        assert!(attempts.borrow().is_empty());

        let outcome = apply_clean_answer(&orphaned, Some("y"), |pid, signal| {
            attempts.borrow_mut().push((pid, signal.to_string()));
            pid == 42
        });
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.killed, vec![42]);
        assert_eq!(outcome.failed, vec![43]);
        assert_eq!(
            attempts.into_inner(),
            vec![(42, "SIGTERM".to_string()), (43, "SIGTERM".to_string())]
        );
    }

    #[test]
    fn command_feedback_messages_match_expected_prompts() {
        let unknown = unknown_command_lines("wat");
        assert!(unknown[0].contains("Unknown command: wat"));
        assert!(unknown[1].contains("Run ports --help for usage."));

        assert!(detail_kill_prompt(3000).contains("Kill process on :3000? [y/N]"));
        assert!(clean_confirmation_prompt().contains("Kill all? [y/N]"));
    }

    #[test]
    fn clean_path_reuses_single_orphaned_snapshot() {
        let calls = RefCell::new(0usize);
        let orphaned = vec![fake_port(3000, 42)];

        let exit = run_clean_with(
            || {
                *calls.borrow_mut() += 1;
                orphaned.clone()
            },
            |_| Some("n".to_string()),
            |_pid, _signal| true,
        );

        assert_eq!(exit, 0);
        assert_eq!(*calls.borrow(), 1);
    }

    #[test]
    fn formats_user_warning_lines_with_indented_prefix() {
        let lines = format_warning_lines(&[
            PortError::CommandMissing("lsof".into()),
            PortError::PermissionDenied("ps".into()),
        ]);

        assert_eq!(
            lines,
            vec![
                "  warning: lsof not found; results may be incomplete".to_string(),
                "  warning: permission denied while inspecting processes".to_string(),
            ]
        );
    }

    #[test]
    fn formats_verbose_entries_as_separate_section() {
        let lines = format_verbose_entries(&[
            "lsof: not found on PATH".to_string(),
            "ps: permission denied".to_string(),
        ]);

        assert_eq!(
            lines,
            vec![
                "".to_string(),
                "  verbose log (2 entries):".to_string(),
                "    lsof: not found on PATH".to_string(),
                "    ps: permission denied".to_string(),
            ]
        );
    }

    #[test]
    fn omits_warning_and_verbose_lines_when_empty() {
        assert!(format_warning_lines(&[]).is_empty());
        assert!(format_verbose_entries(&[]).is_empty());
    }

    #[test]
    fn warning_lines_can_render_real_buffered_errors() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        record_user_warning(&PortError::Timeout {
            cmd: "lsof -iTCP".into(),
            ms: 10_000,
        });

        let drained = drain_user_warnings();
        let lines = format_warning_lines(&drained);

        assert_eq!(
            lines,
            vec!["  warning: system command timed out; results may be incomplete".to_string()]
        );
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn detail_json_output_includes_null_port_when_missing() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        let rendered = render_port_detail_json(39999, None).expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports 39999",
                "ok": true,
                "data": {
                    "port": null,
                },
                "error": null,
            })
        );
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn list_json_output_includes_drained_user_warnings() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        record_user_warning(&PortError::Timeout {
            cmd: "lsof -iTCP".into(),
            ms: 10_000,
        });

        let rendered =
            render_list_json(&[fake_port(3000, 42)]).expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports",
                "ok": true,
                "data": {
                    "ports": [
                        {
                            "port": 3000,
                            "pid": 42,
                            "process_name": "node",
                            "command": "node server.js",
                            "cwd": null,
                            "project_name": null,
                            "framework": null,
                            "uptime": null,
                            "status": "orphaned",
                            "memory": null
                        }
                    ]
                },
                "error": null,
                "warnings": ["system command timed out; results may be incomplete"]
            })
        );
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn ps_json_output_includes_drained_user_warnings() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        record_user_warning(&PortError::Timeout {
            cmd: "ps -ax".into(),
            ms: 10_000,
        });

        let rendered = render_ps_json(&[]).expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports ps",
                "ok": true,
                "data": {
                    "processes": []
                },
                "error": null,
                "warnings": ["system command timed out; results may be incomplete"]
            })
        );
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn detail_json_output_includes_drained_user_warnings() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        record_user_warning(&PortError::Timeout {
            cmd: "lsof -i :39999".into(),
            ms: 10_000,
        });

        let rendered = render_port_detail_json(39999, None).expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports 39999",
                "ok": true,
                "data": {
                    "port": null,
                },
                "error": null,
                "warnings": ["system command timed out; results may be incomplete"]
            })
        );
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn supports_json_for_action_commands() {
        assert!(supports_json(&CliCommand::Clean));
        assert!(supports_json(&CliCommand::Kill(args(&["3000"]))));
        assert!(supports_json(&CliCommand::Logs(args(&["logs", "3000"]))));
        assert!(supports_json(&CliCommand::Help));
        assert!(supports_json(&CliCommand::Watch));
        assert!(supports_json(&CliCommand::Unknown("wat".to_string())));
    }

    #[test]
    fn clean_json_output_is_non_interactive_and_includes_result_lists() {
        let rendered = run_clean_json_with(
            || vec![fake_port(3000, 42), fake_port(3001, 43)],
            |pid, _signal| pid == 42,
        )
        .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered.output).expect("json should parse"),
            json!({
                "command": "ports clean",
                "ok": true,
                "data": {
                    "confirmed": true,
                    "orphaned": [
                        {
                            "port": 3000,
                            "pid": 42,
                            "process_name": "node",
                            "command": "node server.js",
                            "cwd": null,
                            "project_name": null,
                            "framework": null,
                            "uptime": null,
                            "status": "orphaned",
                            "memory": null
                        },
                        {
                            "port": 3001,
                            "pid": 43,
                            "process_name": "node",
                            "command": "node server.js",
                            "cwd": null,
                            "project_name": null,
                            "framework": null,
                            "uptime": null,
                            "status": "orphaned",
                            "memory": null
                        }
                    ],
                    "killed": [42],
                    "failed": [43]
                },
                "error": null
            })
        );
    }

    #[test]
    fn clean_json_output_includes_drained_user_warnings() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        record_user_warning(&PortError::Timeout {
            cmd: "lsof -iTCP".into(),
            ms: 10_000,
        });

        let rendered = run_clean_json_with(|| vec![fake_port(3000, 42)], |_pid, _signal| true)
            .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered.output).expect("json should parse"),
            json!({
                "command": "ports clean",
                "ok": true,
                "data": {
                    "confirmed": true,
                    "orphaned": [
                        {
                            "port": 3000,
                            "pid": 42,
                            "process_name": "node",
                            "command": "node server.js",
                            "cwd": null,
                            "project_name": null,
                            "framework": null,
                            "uptime": null,
                            "status": "orphaned",
                            "memory": null
                        }
                    ],
                    "killed": [42],
                    "failed": []
                },
                "error": null,
                "warnings": ["system command timed out; results may be incomplete"]
            })
        );
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn clean_json_preserves_nonzero_exit_code_when_any_kill_fails() {
        let exit = run_clean_json_with(
            || vec![fake_port(3000, 42), fake_port(3001, 43)],
            |pid, _signal| pid == 42,
        )
        .expect("json render should succeed")
        .exit_code;

        assert_eq!(exit, 1);
    }

    #[test]
    fn rejects_json_for_unsupported_commands() {
        for command in [] {
            assert_eq!(
                dispatch(ParsedCli {
                    show_all: false,
                    verbose: false,
                    json: true,
                    command,
                }),
                1
            );
        }
    }

    #[test]
    fn formats_clear_json_not_supported_message() {
        assert_eq!(
            json_not_supported_message(&CliCommand::Kill(args(&["3000"]))),
            "JSON output is not supported for 'kill' yet.".to_string()
        );
        assert_eq!(
            json_not_supported_message(&CliCommand::Help),
            "JSON output is not supported for 'help' yet.".to_string()
        );
    }

    #[test]
    fn help_json_output_returns_structured_envelope() {
        let rendered = format_help_json().expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports help",
                "ok": true,
                "data": {
                    "usage": [
                        "ports              Show dev server ports",
                        "ports --all        Show all listening ports",
                        "ports ps           Show all running dev processes",
                        "ports ps --all     Show every running process",
                        "ports 3000         Show details for a port",
                        "ports kill 3000       Kill process on port/PID",
                        "ports kill -f 3000    Force kill process on port/PID",
                        "ports kill 3000-3010   Kill a port range",
                        "ports logs 3000       Show logs for port/PID",
                        "ports logs 3000 -f    Follow logs for port/PID",
                        "ports clean          Clean orphaned/zombie processes",
                        "ports watch          Watch port changes",
                        "whoisonport <num> Alias for ports <number>"
                    ]
                },
                "error": null
            })
        );
    }

    #[test]
    fn unknown_command_json_output_returns_structured_error() {
        let rendered = format_unknown_command_json("wat").expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports wat",
                "ok": false,
                "data": null,
                "error": {
                    "code": "unknown_command",
                    "message": "Unknown command: wat"
                }
            })
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
            status: ProcessStatus::Orphaned,
            memory: None,
            git_branch: None,
            process_tree: Vec::new(),
        }
    }
}

fn run_clean() -> i32 {
    run_clean_with(
        scanner::find_orphaned_processes,
        prompt_line,
        kill::kill_process,
    )
}

fn run_clean_json() -> i32 {
    match run_clean_json_with(scanner::find_orphaned_processes, kill::kill_process) {
        Ok(result) => {
            println!("{}", result.output);
            result.exit_code
        }
        Err((message, exit_code)) => {
            eprintln!("{message}");
            exit_code
        }
    }
}

fn run_clean_json_with<FindOrphaned, Kill>(
    find_orphaned: FindOrphaned,
    kill: Kill,
) -> Result<CleanJsonOutput, (String, i32)>
where
    FindOrphaned: Fn() -> Vec<crate::model::PortInfo>,
    Kill: Fn(u32, &str) -> bool,
{
    let orphaned = find_orphaned();
    let outcome = apply_clean_json(&orphaned, kill);
    let warnings = drained_warning_messages();
    json_output::render_json(
        &json_output::CommandEnvelope::ok(
            "ports clean",
            json_output::clean_payload(
                outcome.confirmed,
                &orphaned,
                outcome.killed.clone(),
                outcome.failed.clone(),
            ),
        )
        .with_warnings(warnings),
    )
    .map(|output| CleanJsonOutput {
        output,
        exit_code: outcome.exit_code,
    })
    .map_err(|err| (format!("failed to render json for ports clean: {err}"), 1))
}

#[derive(Debug, Eq, PartialEq)]
struct CleanJsonOutput {
    output: String,
    exit_code: i32,
}

fn run_clean_with<FindOrphaned, Prompt, Kill>(
    find_orphaned: FindOrphaned,
    prompt: Prompt,
    kill: Kill,
) -> i32
where
    FindOrphaned: Fn() -> Vec<crate::model::PortInfo>,
    Prompt: Fn(&str) -> Option<String>,
    Kill: Fn(u32, &str) -> bool,
{
    let orphaned = find_orphaned();
    if orphaned.is_empty() {
        display::display_clean_results(&orphaned, &[], &[]);
        return 0;
    }
    println!();
    println!(
        "{}",
        style::yellow_bold(format!(
            "  Found {} orphaned/zombie process{}:",
            orphaned.len(),
            if orphaned.len() == 1 { "" } else { "es" }
        ))
    );
    for p in &orphaned {
        println!(
            "  {} :{} — {} {}",
            style::gray("•"),
            style::white_bold(p.port.to_string()),
            p.process_name,
            style::gray(format!("(PID {})", p.pid))
        );
    }
    println!();
    let answer = prompt(&clean_confirmation_prompt());
    let outcome = apply_clean_answer(&orphaned, answer.as_deref(), kill);
    if outcome.confirmed {
        display::display_clean_results(&orphaned, &outcome.killed, &outcome.failed);
        return outcome.exit_code;
    }
    println!("{}", style::gray("\n  Aborted.\n"));
    0
}

#[derive(Debug, Eq, PartialEq)]
struct CleanPromptOutcome {
    confirmed: bool,
    killed: Vec<u32>,
    failed: Vec<u32>,
    exit_code: i32,
}

fn apply_clean_json<Kill>(orphaned: &[crate::model::PortInfo], kill: Kill) -> CleanPromptOutcome
where
    Kill: Fn(u32, &str) -> bool,
{
    let mut killed = Vec::new();
    let mut failed = Vec::new();
    for p in orphaned {
        if kill(p.pid, "SIGTERM") {
            killed.push(p.pid);
        } else {
            failed.push(p.pid);
        }
    }
    CleanPromptOutcome {
        confirmed: true,
        exit_code: if failed.is_empty() { 0 } else { 1 },
        killed,
        failed,
    }
}

fn apply_clean_answer<Kill>(
    orphaned: &[crate::model::PortInfo],
    answer: Option<&str>,
    kill: Kill,
) -> CleanPromptOutcome
where
    Kill: Fn(u32, &str) -> bool,
{
    if answer.map(str::to_lowercase).as_deref() != Some("y") {
        return CleanPromptOutcome {
            confirmed: false,
            killed: Vec::new(),
            failed: Vec::new(),
            exit_code: 0,
        };
    }
    let mut killed = Vec::new();
    let mut failed = Vec::new();
    for p in orphaned {
        if kill(p.pid, "SIGTERM") {
            killed.push(p.pid);
        } else {
            failed.push(p.pid);
        }
    }
    CleanPromptOutcome {
        confirmed: true,
        exit_code: if failed.is_empty() { 0 } else { 1 },
        killed,
        failed,
    }
}

fn unknown_command_lines(other: &str) -> [String; 2] {
    [
        style::red(format!("Unknown command: {other}")),
        style::gray("Run ports --help for usage."),
    ]
}

fn help_usage_lines() -> Vec<String> {
    vec![
        "ports              Show dev server ports".to_string(),
        "ports --all        Show all listening ports".to_string(),
        "ports ps           Show all running dev processes".to_string(),
        "ports ps --all     Show every running process".to_string(),
        "ports 3000         Show details for a port".to_string(),
        "ports kill 3000       Kill process on port/PID".to_string(),
        "ports kill -f 3000    Force kill process on port/PID".to_string(),
        "ports kill 3000-3010   Kill a port range".to_string(),
        "ports logs 3000       Show logs for port/PID".to_string(),
        "ports logs 3000 -f    Follow logs for port/PID".to_string(),
        "ports clean          Clean orphaned/zombie processes".to_string(),
        "ports watch          Watch port changes".to_string(),
        "whoisonport <num> Alias for ports <number>".to_string(),
    ]
}

fn format_help_json() -> serde_json::Result<String> {
    json_output::render_json(&json_output::CommandEnvelope::ok(
        "ports help",
        json_output::help_payload(&help_usage_lines()),
    ))
}

fn format_unknown_command_json(other: &str) -> serde_json::Result<String> {
    json_output::render_json(
        &json_output::CommandEnvelope::<json_output::HelpPayload>::err(
            format!("ports {other}"),
            "unknown_command",
            format!("Unknown command: {other}"),
        ),
    )
}

fn detail_kill_prompt(port: u32) -> String {
    style::yellow(format!("  Kill process on :{port}? [y/N] "))
}

fn clean_confirmation_prompt() -> String {
    style::yellow("  Kill all? [y/N] ")
}

fn print_help() {
    println!();
    println!(
        "{}{}",
        style::cyan_bold("  Port Whisperer"),
        style::gray(" — listen to your ports")
    );
    println!();
    println!("{}", style::white("  Usage:"));
    println!(
        "    {}              Show dev server ports",
        style::cyan("ports")
    );
    println!(
        "    {}        Show all listening ports",
        style::cyan("ports --all")
    );
    println!(
        "    {}           Show all running dev processes",
        style::cyan("ports ps")
    );
    println!(
        "    {}     Show every running process",
        style::cyan("ports ps --all")
    );
    println!(
        "    {}        Show details for a port",
        style::cyan("ports 3000")
    );
    println!(
        "    {}       Kill process on port/PID",
        style::cyan("ports kill 3000")
    );
    println!(
        "    {}    Force kill process on port/PID",
        style::cyan("ports kill -f 3000")
    );
    println!(
        "    {}   Kill a port range",
        style::cyan("ports kill 3000-3010")
    );
    println!(
        "    {}       Show logs for port/PID",
        style::cyan("ports logs 3000")
    );
    println!(
        "    {}    Follow logs for port/PID",
        style::cyan("ports logs 3000 -f")
    );
    println!(
        "    {}          Clean orphaned/zombie processes",
        style::cyan("ports clean")
    );
    println!(
        "    {}          Watch port changes",
        style::cyan("ports watch")
    );
    println!(
        "    {} Alias for ports <number>",
        style::cyan("whoisonport <num>")
    );
    println!();
}
