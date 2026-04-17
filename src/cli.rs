use crate::display;
use crate::kill;
use crate::logs;
use crate::scanner;
use crate::style;
use crate::util::prompt_line;
use crate::watch;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedCli {
    pub show_all: bool,
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

    match parsed.command {
        CliCommand::List => {
            let mut ports = scanner::get_listening_ports(false);
            if !parsed.show_all {
                ports.retain(|p| scanner::is_dev_process(&p.process_name, &p.command));
            }
            display::display_port_table(&ports, !parsed.show_all);
            0
        }
        CliCommand::Help => {
            print_help();
            0
        }
        CliCommand::Ps => {
            let mut processes = scanner::get_all_processes();
            if !parsed.show_all {
                processes.retain(|p| scanner::is_dev_process(&p.process_name, &p.command));
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
            display::display_process_table(&processes, !parsed.show_all);
            0
        }
        CliCommand::Clean => run_clean(),
        CliCommand::Kill(args) => kill::run_kill(&args),
        CliCommand::Logs(args) => logs::run_logs(&args),
        CliCommand::Watch => watch::run_watch(),
        CliCommand::PortDetail(port) => run_port_detail(port),
        CliCommand::Unknown(other) => {
            for line in unknown_command_lines(&other) {
                println!("{line}");
            }
            1
        }
    }
}

pub fn parse_args(args: &[String]) -> ParsedCli {
    let show_all = args.iter().any(|a| a == "--all" || a == "-a");
    let filtered_args: Vec<String> = args
        .iter()
        .filter(|a| a.as_str() != "--all" && a.as_str() != "-a")
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
    ParsedCli { show_all, command }
}

fn run_port_detail(port: u32) -> i32 {
    let info = if port <= u16::MAX as u32 {
        scanner::get_port_details(port as u16)
    } else {
        None
    };
    display::display_port_detail(info.as_ref());
    if let Some(info) = info {
        let prompt = detail_kill_prompt(port);
        if let Some(answer) = prompt_line(&prompt) {
            if answer.to_lowercase() == "y" {
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
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{
        CliCommand, ParsedCli, apply_clean_answer, clean_confirmation_prompt, detail_kill_prompt,
        parse_args, unknown_command_lines,
    };
    use crate::model::{PortInfo, ProcessStatus};
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
                command: CliCommand::List
            }
        );
        assert_eq!(
            parse_args(&args(&["ps", "-a"])),
            ParsedCli {
                show_all: true,
                command: CliCommand::Ps
            }
        );
        assert_eq!(
            parse_args(&args(&["kill", "--all", "3000"])),
            ParsedCli {
                show_all: true,
                command: CliCommand::Kill(args(&["3000"]))
            }
        );
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
    let orphaned = scanner::find_orphaned_processes();
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
    let answer = prompt_line(&clean_confirmation_prompt());
    let outcome = apply_clean_answer(&orphaned, answer.as_deref(), kill::kill_process);
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
