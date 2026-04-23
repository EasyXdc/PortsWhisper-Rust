use crate::check;
use crate::cli_args;
use crate::display;
use crate::error;
use crate::json_output;
use crate::kill;
use crate::logs;
use crate::open;
use crate::ports;
use crate::scanner;
use crate::style;
use crate::util::prompt_line;
use crate::watch;
use clap_complete::{Shell, generate};
use std::time::Instant;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedCli {
    pub show_all: bool,
    pub quiet: bool,
    pub ascii: bool,
    pub verbose: bool,
    pub json: bool,
    pub command: CliCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CliCommand {
    List(QueryFilters),
    Help,
    Completion(Shell),
    Check(Vec<u16>),
    Open(u16),
    Ps(QueryFilters),
    Clean,
    Kill(Vec<String>),
    Logs(Vec<String>),
    Watch,
    PortDetail(u32, QueryFilters),
    PortRange(PortRange, QueryFilters),
    Unknown(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QueryFilters {
    pub framework: Option<String>,
    pub pid: Option<u32>,
    pub project: Option<String>,
    pub port_range: Option<PortRange>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

pub fn run(binary_name: &str, args: Vec<String>) -> i32 {
    let parsed = parse_args(binary_name, &args);
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
    style::set_force_ascii(parsed.ascii);
    let display_config = display::display_config(parsed.quiet, parsed.ascii, parsed.json);

    if parsed.json && !supports_json(&parsed.command) {
        eprintln!("{}", json_not_supported_message(&parsed.command));
        return 1;
    }

    match parsed.command {
        CliCommand::List(filters) => {
            let started_at = Instant::now();
            let mut ports = ports::get_listening_ports(false);
            if !parsed.show_all {
                ports.retain(|p| scanner::is_dev_process(&p.process_name, &p.command));
            }
            ports = filter_ports(ports, &filters);
            if parsed.json {
                return json_output::print_json_output(render_list_json(&ports, &filters));
            }
            let display_config =
                display::with_command_elapsed(&display_config, started_at.elapsed());
            display::display_port_table_with_config(&ports, !parsed.show_all, &display_config);
            print_warning_lines(&error::drain_user_warnings());
            0
        }
        CliCommand::Help => {
            if parsed.json {
                return json_output::print_json_output(format_help_json());
            }
            print_help();
            0
        }
        CliCommand::Completion(shell) => run_completion(shell),
        CliCommand::Check(ports) => {
            if parsed.json {
                return check::run_check_json(&ports);
            }
            check::run_check(&ports)
        }
        CliCommand::Open(port) => open::run_open(port),
        CliCommand::Ps(filters) => {
            let mut processes = if parsed.show_all {
                scanner::get_all_processes()
            } else {
                scanner::get_all_dev_processes()
            };
            processes = filter_processes(processes, &filters);
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
                return json_output::print_json_output(render_ps_json(&processes, &filters));
            }
            display::display_process_table_with_config(
                &processes,
                !parsed.show_all,
                &display_config,
            );
            print_warning_lines(&error::drain_user_warnings());
            0
        }
        CliCommand::Clean => {
            if parsed.json {
                return run_clean_json();
            }
            run_clean(&display_config)
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
            watch::run_watch(display_config)
        }
        CliCommand::PortDetail(port, filters) => {
            run_port_detail(port, &filters, parsed.json, &display_config)
        }
        CliCommand::PortRange(range, mut filters) => {
            filters.port_range = Some(range);
            let mut ports = ports::get_listening_ports(false);
            if !parsed.show_all {
                ports.retain(|p| scanner::is_dev_process(&p.process_name, &p.command));
            }
            ports = filter_ports(ports, &filters);
            if parsed.json {
                return json_output::print_json_output(render_list_json(&ports, &filters));
            }
            display::display_port_table_with_config(&ports, !parsed.show_all, &display_config);
            print_warning_lines(&error::drain_user_warnings());
            0
        }
        CliCommand::Unknown(other) => {
            if parsed.json {
                let exit = json_output::print_json_output(format_unknown_command_json(&other));
                return if exit == 0 { 1 } else { exit };
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
        CliCommand::List(_)
            | CliCommand::Help
            | CliCommand::Ps(_)
            | CliCommand::Check(_)
            | CliCommand::Clean
            | CliCommand::Kill(_)
            | CliCommand::Logs(_)
            | CliCommand::Watch
            | CliCommand::PortDetail(_, _)
            | CliCommand::PortRange(_, _)
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
        CliCommand::List(_) => "ports",
        CliCommand::Help => "help",
        CliCommand::Completion(_) => "completion",
        CliCommand::Check(_) => "check",
        CliCommand::Open(_) => "open",
        CliCommand::Ps(_) => "ps",
        CliCommand::Clean => "clean",
        CliCommand::Kill(_) => "kill",
        CliCommand::Logs(_) => "logs",
        CliCommand::Watch => "watch",
        CliCommand::PortDetail(_, _) => "detail",
        CliCommand::PortRange(_, _) => "ports",
        CliCommand::Unknown(_) => "unknown",
    }
}

pub fn parse_args(binary_name: &str, args: &[String]) -> ParsedCli {
    let parsed_args = cli_args::parse(binary_name, args).unwrap_or_else(|err| err.exit());
    let command = parse_command(
        binary_name,
        &parsed_args.remaining_args,
        parsed_args.completion_shell,
    );
    ParsedCli {
        show_all: parsed_args.show_all,
        quiet: parsed_args.quiet,
        ascii: parsed_args.ascii,
        verbose: parsed_args.verbose,
        json: parsed_args.json,
        command,
    }
}

fn parse_command(
    binary_name: &str,
    args: &[String],
    completion_shell: Option<Shell>,
) -> CliCommand {
    if binary_name == "whoisonport" {
        return parse_alias_command(args);
    }

    match args.first().map(String::as_str) {
        None => CliCommand::List(parse_query_filters(args)),
        Some("help" | "--help" | "-h") => CliCommand::Help,
        Some("completion") => CliCommand::Completion(
            completion_shell.expect("completion subcommand should be validated by cli_args::parse"),
        ),
        Some("check") => CliCommand::Check(parse_check_ports(&args[1..])),
        Some("open") => CliCommand::Open(parse_open_port(&args[1..])),
        Some("ps") => CliCommand::Ps(parse_query_filters(&args[1..])),
        Some("clean") => CliCommand::Clean,
        Some("kill") => CliCommand::Kill(args[1..].to_vec()),
        Some("logs") => CliCommand::Logs(args.to_vec()),
        Some("watch") => CliCommand::Watch,
        Some(other) => {
            if let Some(range) = parse_port_range(other) {
                CliCommand::PortRange(range, parse_query_filters(&args[1..]))
            } else if let Ok(port) = other.parse::<u32>() {
                CliCommand::PortDetail(port, parse_query_filters(&args[1..]))
            } else if is_query_filter_flag(other) {
                CliCommand::List(parse_query_filters(args))
            } else {
                CliCommand::Unknown(other.to_string())
            }
        }
    }
}

fn parse_alias_command(args: &[String]) -> CliCommand {
    match args {
        [] => CliCommand::Help,
        [single] if matches!(single.as_str(), "help" | "--help" | "-h") => CliCommand::Help,
        [single] => {
            if let Ok(port) = single.parse::<u32>() {
                CliCommand::PortDetail(port, QueryFilters::default())
            } else {
                CliCommand::Unknown(single.to_string())
            }
        }
        [first, ..] => CliCommand::Unknown(first.to_string()),
    }
}

fn run_completion(shell: Shell) -> i32 {
    let mut command = cli_args::command("ports");
    let mut stdout = std::io::stdout();
    generate(shell, &mut command, "ports", &mut stdout);
    0
}

fn parse_query_filters(args: &[String]) -> QueryFilters {
    let mut filters = QueryFilters::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--framework" => {
                filters.framework = args.get(index + 1).cloned();
                index += 2;
            }
            "--pid" => {
                filters.pid = args
                    .get(index + 1)
                    .and_then(|value| value.parse::<u32>().ok());
                index += 2;
            }
            "--project" => {
                filters.project = args.get(index + 1).cloned();
                index += 2;
            }
            "--port-range" => {
                filters.port_range = args
                    .get(index + 1)
                    .and_then(|value| parse_port_range(value));
                index += 2;
            }
            other => {
                if filters.port_range.is_none() {
                    filters.port_range = parse_port_range(other);
                }
                index += 1;
            }
        }
    }
    filters
}

fn parse_check_ports(args: &[String]) -> Vec<u16> {
    args.iter()
        .map(|value| {
            value
                .parse::<u16>()
                .expect("check ports should be validated by cli_args::parse")
        })
        .collect()
}

fn parse_open_port(args: &[String]) -> u16 {
    args.first()
        .expect("open port should be validated by cli_args::parse")
        .parse::<u16>()
        .expect("open port should be validated by cli_args::parse")
}

fn is_query_filter_flag(arg: &str) -> bool {
    matches!(arg, "--framework" | "--pid" | "--project" | "--port-range")
}

fn parse_port_range(value: &str) -> Option<PortRange> {
    let (start, end) = value.split_once('-')?;
    let start = start.parse::<u16>().ok()?;
    let end = end.parse::<u16>().ok()?;
    if start > end {
        return None;
    }
    Some(PortRange { start, end })
}

fn filter_ports(
    ports: Vec<crate::model::PortInfo>,
    filters: &QueryFilters,
) -> Vec<crate::model::PortInfo> {
    ports
        .into_iter()
        .filter(|port| matches_port_filters(port, filters))
        .collect()
}

fn filter_processes(
    processes: Vec<crate::model::ProcessInfo>,
    filters: &QueryFilters,
) -> Vec<crate::model::ProcessInfo> {
    let allowed_ports = filters.port_range.map(|range| {
        ports::get_listening_ports(false)
            .into_iter()
            .filter(|port| port.port >= range.start && port.port <= range.end)
            .map(|port| port.pid)
            .collect::<std::collections::HashSet<_>>()
    });

    processes
        .into_iter()
        .filter(|process| matches_process_filters(process, filters, allowed_ports.as_ref()))
        .collect()
}

fn matches_port_filters(port: &crate::model::PortInfo, filters: &QueryFilters) -> bool {
    matches_name_filter(port.framework.as_deref(), filters.framework.as_deref())
        && matches_name_filter(port.project_name.as_deref(), filters.project.as_deref())
        && filters.pid.is_none_or(|pid| port.pid == pid)
        && filters
            .port_range
            .is_none_or(|range| port.port >= range.start && port.port <= range.end)
}

fn matches_process_filters(
    process: &crate::model::ProcessInfo,
    filters: &QueryFilters,
    allowed_pids: Option<&std::collections::HashSet<u32>>,
) -> bool {
    matches_name_filter(process.framework.as_deref(), filters.framework.as_deref())
        && matches_name_filter(process.project_name.as_deref(), filters.project.as_deref())
        && filters.pid.is_none_or(|pid| process.pid == pid)
        && allowed_pids.is_none_or(|pids| pids.contains(&process.pid))
}

fn matches_name_filter(value: Option<&str>, filter: Option<&str>) -> bool {
    match filter {
        None => true,
        Some(filter) => value.is_some_and(|value| value.eq_ignore_ascii_case(filter)),
    }
}

fn run_port_detail(
    port: u32,
    filters: &QueryFilters,
    json: bool,
    display_config: &display::DisplayConfig,
) -> i32 {
    let info = if port <= u16::MAX as u32 {
        ports::get_port_details(port as u16)
    } else {
        None
    }
    .filter(|info| matches_port_filters(info, filters));
    if json {
        return json_output::print_json_output(render_port_detail_json(
            port,
            info.as_ref(),
            filters,
        ));
    }
    display::display_port_detail_with_config(info.as_ref(), display_config);
    print_warning_lines(&error::drain_user_warnings());
    if let Some(info) = info {
        let prompt = detail_kill_prompt(port);
        if let Some(answer) = prompt_line(&prompt)
            && answer.to_lowercase() == "y"
        {
            if kill::kill_process(info.pid, "SIGTERM") {
                println!("{}", render_kill_feedback(info.pid, true));
            } else {
                println!("{}", render_kill_feedback(info.pid, false));
                return 1;
            }
        }
    }
    0
}

fn render_kill_feedback(pid: u32, success: bool) -> String {
    let glyphs = style::glyphs();
    if success {
        style::green(format!("\n  {} Killed PID {}\n", glyphs.success, pid))
    } else {
        style::red(format!(
            "\n  {} Failed. Try: sudo kill -9 {}\n",
            glyphs.failure, pid
        ))
    }
}

fn render_port_detail_json(
    port: u32,
    info: Option<&crate::model::PortInfo>,
    filters: &QueryFilters,
) -> serde_json::Result<String> {
    render_query_json(
        query_command_name(format!("ports {port}"), filters),
        json_output::detail_payload(info),
    )
}

fn render_list_json(
    ports: &[crate::model::PortInfo],
    filters: &QueryFilters,
) -> serde_json::Result<String> {
    render_query_json(
        query_command_name("ports", filters),
        json_output::list_payload(ports),
    )
}

fn render_ps_json(
    processes: &[crate::model::ProcessInfo],
    filters: &QueryFilters,
) -> serde_json::Result<String> {
    render_query_json(
        query_command_name("ports ps", filters),
        json_output::process_list_payload(processes),
    )
}

fn query_command_name(base: impl Into<String>, filters: &QueryFilters) -> String {
    let mut command = base.into();
    if let Some(pid) = filters.pid {
        command.push_str(&format!(" --pid {pid}"));
    }
    if let Some(project) = &filters.project {
        command.push_str(&format!(" --project {project}"));
    }
    if let Some(framework) = &filters.framework {
        command.push_str(&format!(" --framework {framework}"));
    }
    if let Some(range) = filters.port_range {
        command.push_str(&format!(" --port-range {}-{}", range.start, range.end));
    }
    command
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
        CliCommand, ParsedCli, PortRange, QueryFilters, apply_clean_answer,
        clean_confirmation_prompt, detail_kill_prompt, dispatch, format_help_json,
        format_unknown_command_json, format_verbose_entries, format_warning_lines,
        help_usage_lines, json_not_supported_message, parse_args, render_kill_feedback,
        render_list_json, render_port_detail_json, render_ps_json, run_clean_json_with,
        run_clean_with, supports_json, unknown_command_lines,
    };
    use crate::cli_args;
    use crate::display::DisplayConfig;
    use crate::error::{PortError, drain_user_warnings, record_user_warning, verbose_test_lock};
    use crate::model::{PortInfo, ProcessInfo, ProcessStatus};
    use clap_complete::Shell;
    use serde_json::json;
    use std::cell::RefCell;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn parses_public_commands_without_scanning() {
        assert_eq!(
            parse_args("ports", &args(&[])),
            ParsedCli {
                show_all: false,
                quiet: false,
                ascii: false,
                verbose: false,
                json: false,
                command: CliCommand::List(QueryFilters::default())
            }
        );
        assert_eq!(
            parse_args("ports", &args(&["help"])).command,
            CliCommand::Help
        );
        assert_eq!(
            parse_args("ports", &args(&["--help"])).command,
            CliCommand::Help
        );
        assert_eq!(
            parse_args("ports", &args(&["-h"])).command,
            CliCommand::Help
        );
        assert_eq!(
            parse_args("ports", &args(&["ps"])).command,
            CliCommand::Ps(QueryFilters::default())
        );
        assert_eq!(
            parse_args("ports", &args(&["clean"])).command,
            CliCommand::Clean
        );
        assert_eq!(
            parse_args("ports", &args(&["watch"])).command,
            CliCommand::Watch
        );
        assert_eq!(
            parse_args("ports", &args(&["check", "3000", "5173"])).command,
            CliCommand::Check(vec![3000, 5173])
        );
        assert_eq!(
            parse_args("ports", &args(&["open", "3000"])).command,
            CliCommand::Open(3000)
        );
        assert_eq!(
            parse_args("ports", &args(&["completion", "bash"])).command,
            CliCommand::Completion(Shell::Bash)
        );
        assert_eq!(
            parse_args("ports", &args(&["3000"])).command,
            CliCommand::PortDetail(3000, QueryFilters::default())
        );
    }

    #[test]
    fn normalizes_all_flags_globally_before_command_dispatch() {
        assert_eq!(
            parse_args("ports", &args(&["--all"])),
            ParsedCli {
                show_all: true,
                quiet: false,
                ascii: false,
                verbose: false,
                json: false,
                command: CliCommand::List(QueryFilters::default())
            }
        );
        assert_eq!(
            parse_args("ports", &args(&["ps", "-a"])),
            ParsedCli {
                show_all: true,
                quiet: false,
                ascii: false,
                verbose: false,
                json: false,
                command: CliCommand::Ps(QueryFilters::default())
            }
        );
        assert_eq!(
            parse_args("ports", &args(&["kill", "--all", "3000"])),
            ParsedCli {
                show_all: true,
                quiet: false,
                ascii: false,
                verbose: false,
                json: false,
                command: CliCommand::Kill(args(&["3000"]))
            }
        );
    }

    #[test]
    fn recognizes_quiet_and_ascii_flags_globally_and_filters_them_out() {
        let parsed = parse_args("ports", &args(&["--quiet", "--ascii"]));
        assert!(parsed.quiet);
        assert!(parsed.ascii);
        assert!(!parsed.show_all);
        assert_eq!(parsed.command, CliCommand::List(QueryFilters::default()));

        let parsed = parse_args("ports", &args(&["ps", "--ascii", "--quiet", "--all"]));
        assert!(parsed.quiet);
        assert!(parsed.ascii);
        assert!(parsed.show_all);
        assert_eq!(parsed.command, CliCommand::Ps(QueryFilters::default()));

        let parsed = parse_args("ports", &args(&["ps"]));
        assert!(!parsed.quiet);
        assert!(!parsed.ascii);
    }

    #[test]
    fn recognizes_verbose_flag_globally_and_filters_it_out() {
        let parsed = parse_args("ports", &args(&["--verbose"]));
        assert!(parsed.verbose);
        assert!(!parsed.json);
        assert_eq!(parsed.command, CliCommand::List(QueryFilters::default()));

        let parsed = parse_args("ports", &args(&["ps", "--verbose", "--all"]));
        assert!(parsed.verbose);
        assert!(parsed.show_all);
        assert_eq!(parsed.command, CliCommand::Ps(QueryFilters::default()));

        let parsed = parse_args("ports", &args(&["kill", "--verbose", "3000"]));
        assert!(parsed.verbose);
        assert_eq!(parsed.command, CliCommand::Kill(args(&["3000"])));

        assert!(!parse_args("ports", &args(&["ps"])).verbose);
    }

    #[test]
    fn recognizes_json_flag_globally_and_filters_it_out() {
        let parsed = parse_args("ports", &args(&["--json"]));
        assert!(parsed.json);
        assert_eq!(parsed.command, CliCommand::List(QueryFilters::default()));

        let parsed = parse_args("ports", &args(&["ps", "--json", "--all"]));
        assert!(parsed.json);
        assert!(parsed.show_all);
        assert_eq!(parsed.command, CliCommand::Ps(QueryFilters::default()));

        let parsed = parse_args("ports", &args(&["kill", "--json", "3000"]));
        assert!(parsed.json);
        assert_eq!(parsed.command, CliCommand::Kill(args(&["3000"])));

        assert!(!parse_args("ports", &args(&["ps"])).json);
    }

    #[test]
    fn keeps_command_specific_arguments_after_normalization() {
        assert_eq!(
            parse_args("ports", &args(&["kill", "-f", "3000", "3001"])).command,
            CliCommand::Kill(args(&["-f", "3000", "3001"]))
        );
        assert_eq!(
            parse_args("ports", &args(&["kill", "3000", "--ascii"])).command,
            CliCommand::Kill(args(&["3000", "--ascii"]))
        );
        assert_eq!(
            parse_args("ports", &args(&["logs", "3000", "--lines=5", "-f"])).command,
            CliCommand::Logs(args(&["logs", "3000", "--lines=5", "-f"]))
        );
        assert_eq!(
            parse_args("ports", &args(&["unknown"])).command,
            CliCommand::Unknown("unknown".to_string())
        );
    }

    #[test]
    fn clap_parser_filters_global_flags_and_preserves_remaining_argv() {
        let parsed = cli_args::parse(
            "ports",
            &args(&["--json", "logs", "3000", "--lines=5", "-f"]),
        )
        .expect("clap parser should accept current logs contract");

        assert!(parsed.json);
        assert!(!parsed.show_all);
        assert!(!parsed.verbose);
        assert_eq!(
            parsed.remaining_args,
            args(&["logs", "3000", "--lines=5", "-f"])
        );

        let parsed = cli_args::parse("ports", &args(&["3000", "--all", "--verbose"]))
            .expect("clap parser should keep positional detail behavior");

        assert!(parsed.show_all);
        assert!(parsed.verbose);
        assert_eq!(parsed.remaining_args, args(&["3000"]));

        let parsed = cli_args::parse("ports", &args(&["kill", "--quiet", "3000", "--ascii"]))
            .expect("kill should keep leading display flags global and trailing tokens as payload");

        assert!(parsed.quiet);
        assert!(!parsed.ascii);
        assert_eq!(parsed.remaining_args, args(&["kill", "3000", "--ascii"]));
    }

    #[test]
    fn parses_query_filters_for_ports_and_ps_commands() {
        assert_eq!(
            parse_args(
                "ports",
                &args(&[
                    "--framework",
                    "nextjs",
                    "--project",
                    "demo",
                    "--pid",
                    "42",
                    "--port-range",
                    "3000-3010",
                ])
            )
            .command,
            CliCommand::List(QueryFilters {
                framework: Some("nextjs".to_string()),
                pid: Some(42),
                project: Some("demo".to_string()),
                port_range: Some(PortRange {
                    start: 3000,
                    end: 3010
                }),
            })
        );

        assert_eq!(
            parse_args(
                "ports",
                &args(&[
                    "ps",
                    "--framework",
                    "nextjs",
                    "--project",
                    "demo",
                    "--pid",
                    "42",
                ])
            )
            .command,
            CliCommand::Ps(QueryFilters {
                framework: Some("nextjs".to_string()),
                pid: Some(42),
                project: Some("demo".to_string()),
                port_range: None,
            })
        );
    }

    #[test]
    fn parses_positional_port_range_as_query_path() {
        assert_eq!(
            parse_args("ports", &args(&["3000-3010"])).command,
            CliCommand::PortRange(
                PortRange {
                    start: 3000,
                    end: 3010
                },
                QueryFilters::default()
            )
        );
    }

    #[test]
    fn rejects_duplicate_range_sources() {
        let error = cli_args::parse("ports", &args(&["3000-3010", "--port-range", "4000-4010"]))
            .expect_err("duplicate range sources should be rejected");
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn rejects_invalid_positional_ranges_with_query_error() {
        let error = cli_args::parse("ports", &args(&["4000-3000"]))
            .expect_err("reversed positional range should fail fast");

        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn filters_port_queries_by_all_supported_query_filters() {
        let filters = QueryFilters {
            framework: Some("nextjs".to_string()),
            pid: Some(42),
            project: Some("demo".to_string()),
            port_range: Some(PortRange {
                start: 3000,
                end: 3010,
            }),
        };

        let ports = vec![
            fake_port_with_filters(3000, 42, Some("demo"), Some("NextJs")),
            fake_port_with_filters(3001, 42, Some("demo"), Some("Vite")),
            fake_port_with_filters(3002, 43, Some("demo"), Some("NextJs")),
            fake_port_with_filters(4000, 42, Some("demo"), Some("NextJs")),
            fake_port_with_filters(3003, 42, Some("other"), Some("NextJs")),
        ];

        assert_eq!(
            super::filter_ports(ports, &filters)
                .into_iter()
                .map(|port| port.port)
                .collect::<Vec<_>>(),
            vec![3000]
        );
    }

    #[test]
    fn filters_process_queries_by_all_supported_query_filters() {
        let filters = QueryFilters {
            framework: Some("node.js".to_string()),
            pid: Some(42),
            project: Some("demo".to_string()),
            port_range: None,
        };

        let processes = vec![
            fake_process(42, Some("demo"), Some("Node.js")),
            fake_process(42, Some("demo"), Some("Vite")),
            fake_process(43, Some("demo"), Some("Node.js")),
            fake_process(42, Some("other"), Some("Node.js")),
        ];

        assert_eq!(
            super::filter_processes(processes, &filters)
                .into_iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            vec![42]
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
    fn ascii_kill_feedback_uses_ascii_markers() {
        let _guard = crate::style::glyph_test_lock().lock().unwrap();
        crate::style::set_force_ascii(true);
        let success = render_kill_feedback(42, true);
        let failure = render_kill_feedback(42, false);
        crate::style::set_force_ascii(false);

        assert!(
            success.contains("  v Killed PID 42"),
            "expected ascii success marker: {success}"
        );
        assert!(
            failure.contains("  x Failed. Try: sudo kill -9 42"),
            "expected ascii failure marker: {failure}"
        );
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
            &DisplayConfig::default(),
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
        let rendered = render_port_detail_json(39999, None, &QueryFilters::default())
            .expect("json render should succeed");

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

        let rendered = render_list_json(&[fake_port(3000, 42)], &QueryFilters::default())
            .expect("json render should succeed");

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

        let rendered =
            render_ps_json(&[], &QueryFilters::default()).expect("json render should succeed");

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

        let rendered = render_port_detail_json(39999, None, &QueryFilters::default())
            .expect("json render should succeed");

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
        assert!(!supports_json(&CliCommand::Open(3000)));
    }

    #[test]
    fn clean_json_output_is_non_interactive_and_includes_result_lists() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();

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
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();

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
        let command = CliCommand::Open(3000);
        assert_eq!(
            dispatch(ParsedCli {
                show_all: false,
                quiet: false,
                ascii: false,
                verbose: false,
                json: true,
                command,
            }),
            1
        );
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
        assert_eq!(
            json_not_supported_message(&CliCommand::Open(3000)),
            "JSON output is not supported for 'open' yet.".to_string()
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
                        "ports --framework <name>  Filter ports by framework",
                        "ports --pid <pid>         Filter ports by PID",
                        "ports --project <name>    Filter ports by project",
                        "ports --port-range <start-end> Filter ports by port range",
                        "ports ps           Show all running dev processes",
                        "ports ps --all     Show every running process",
                        "ports ps --framework <name> Filter processes by framework",
                        "ports ps --pid <pid>        Filter processes by PID",
                        "ports ps --project <name>   Filter processes by project",
                        "ports 3000         Show details for a port",
                        "ports kill 3000       Kill process on port/PID",
                        "ports kill -f 3000    Force kill process on port/PID",
                        "ports kill 3000-3010   Kill a port range",
                        "ports logs 3000       Show logs for port/PID",
                        "ports logs 3000 -f    Follow logs for port/PID",
                        "ports check 3000 5173 Check whether ports are occupied",
                        "ports open 3000       Open a localhost URL in your browser",
                        "ports clean          Clean orphaned/zombie processes",
                        "ports watch          Watch port changes",
                        "ports completion <shell> Generate shell completion script",
                        "whoisonport <num> Alias for ports <number>"
                    ]
                },
                "error": null
            })
        );
    }

    #[test]
    fn help_usage_surface_includes_completion_and_query_filters() {
        let usage = help_usage_lines();

        assert!(
            usage
                .iter()
                .any(|line| line.contains("ports completion <shell>"))
        );
        assert!(usage.iter().any(|line| line.contains("--framework")));
        assert!(usage.iter().any(|line| line.contains("--pid")));
        assert!(usage.iter().any(|line| line.contains("--project")));
        assert!(usage.iter().any(|line| line.contains("--port-range")));
        assert!(usage.iter().any(|line| line.contains("ports open 3000")));
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

    fn fake_port_with_filters(
        port: u16,
        pid: u32,
        project: Option<&str>,
        framework: Option<&str>,
    ) -> PortInfo {
        let mut info = fake_port(port, pid);
        info.project_name = project.map(str::to_string);
        info.framework = framework.map(str::to_string);
        info
    }

    fn fake_process(pid: u32, project: Option<&str>, framework: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid: Some(1),
            process_name: "node".to_string(),
            command: "node server.js".to_string(),
            description: "server.js".to_string(),
            cpu: 1.0,
            rss_kb: 1024,
            memory: Some("1.0 MB".to_string()),
            cwd: None,
            project_name: project.map(str::to_string),
            framework: framework.map(str::to_string),
            uptime: None,
            status_raw: String::new(),
        }
    }
}

fn run_clean(display_config: &display::DisplayConfig) -> i32 {
    run_clean_with(
        scanner::find_orphaned_processes,
        prompt_line,
        kill::kill_process,
        display_config,
    )
}

fn run_clean_json() -> i32 {
    let orphaned = scanner::find_orphaned_processes();
    let outcome = apply_clean_json(&orphaned, kill::kill_process);
    let warnings = drained_warning_messages();
    let exit_code = outcome.exit_code;
    let json_exit = json_output::print_json_output(json_output::render_json(
        &json_output::CommandEnvelope::ok(
            "ports clean",
            json_output::clean_payload(
                outcome.confirmed,
                &orphaned,
                outcome.killed,
                outcome.failed,
            ),
        )
        .with_warnings(warnings),
    ));
    if json_exit != 0 { json_exit } else { exit_code }
}

#[allow(dead_code)]
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
#[allow(dead_code)]
struct CleanJsonOutput {
    output: String,
    exit_code: i32,
}

fn run_clean_with<FindOrphaned, Prompt, Kill>(
    find_orphaned: FindOrphaned,
    prompt: Prompt,
    kill: Kill,
    display_config: &display::DisplayConfig,
) -> i32
where
    FindOrphaned: Fn() -> Vec<crate::model::PortInfo>,
    Prompt: Fn(&str) -> Option<String>,
    Kill: Fn(u32, &str) -> bool,
{
    let orphaned = find_orphaned();
    if orphaned.is_empty() {
        display::display_clean_results_with_config(&orphaned, &[], &[], display_config);
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
        let glyphs = style::glyphs();
        println!(
            "  {} :{} — {} {}",
            style::gray(glyphs.bullet),
            style::white_bold(p.port.to_string()),
            p.process_name,
            style::gray(format!("(PID {})", p.pid))
        );
    }
    println!();
    let answer = prompt(&clean_confirmation_prompt());
    let outcome = apply_clean_answer(&orphaned, answer.as_deref(), kill);
    if outcome.confirmed {
        display::display_clean_results_with_config(
            &orphaned,
            &outcome.killed,
            &outcome.failed,
            display_config,
        );
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
        "ports --framework <name>  Filter ports by framework".to_string(),
        "ports --pid <pid>         Filter ports by PID".to_string(),
        "ports --project <name>    Filter ports by project".to_string(),
        "ports --port-range <start-end> Filter ports by port range".to_string(),
        "ports ps           Show all running dev processes".to_string(),
        "ports ps --all     Show every running process".to_string(),
        "ports ps --framework <name> Filter processes by framework".to_string(),
        "ports ps --pid <pid>        Filter processes by PID".to_string(),
        "ports ps --project <name>   Filter processes by project".to_string(),
        "ports 3000         Show details for a port".to_string(),
        "ports kill 3000       Kill process on port/PID".to_string(),
        "ports kill -f 3000    Force kill process on port/PID".to_string(),
        "ports kill 3000-3010   Kill a port range".to_string(),
        "ports logs 3000       Show logs for port/PID".to_string(),
        "ports logs 3000 -f    Follow logs for port/PID".to_string(),
        "ports check 3000 5173 Check whether ports are occupied".to_string(),
        "ports open 3000       Open a localhost URL in your browser".to_string(),
        "ports clean          Clean orphaned/zombie processes".to_string(),
        "ports watch          Watch port changes".to_string(),
        "ports completion <shell> Generate shell completion script".to_string(),
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
    for line in help_usage_lines() {
        println!("    {}", line);
    }
    println!();
}
