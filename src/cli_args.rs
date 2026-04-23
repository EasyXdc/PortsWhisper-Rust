use clap::builder::EnumValueParser;
use clap::{Arg, ArgAction, Command, Parser};
use clap_complete::Shell;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedCliArgs {
    pub binary_name: String,
    pub show_all: bool,
    pub quiet: bool,
    pub ascii: bool,
    pub verbose: bool,
    pub json: bool,
    pub completion_shell: Option<Shell>,
    pub remaining_args: Vec<String>,
}

#[derive(Parser, Debug)]
#[command(disable_help_flag = true, disable_version_flag = true)]
struct GlobalFlags {
    #[arg(short = 'a', long = "all")]
    show_all: bool,
    #[arg(long = "quiet")]
    quiet: bool,
    #[arg(long = "ascii")]
    ascii: bool,
    #[arg(long = "verbose")]
    verbose: bool,
    #[arg(long = "json")]
    json: bool,
}

pub fn parse(binary_name: &str, args: &[String]) -> Result<ParsedCliArgs, clap::Error> {
    let mut flag_args = vec![binary_name.to_string()];
    let mut remaining_args = Vec::new();
    let mut kill_targets_started = false;

    for (index, arg) in args.iter().enumerate() {
        if is_global_flag(arg) && !is_kill_display_target(args, index, arg, kill_targets_started) {
            flag_args.push(arg.clone());
        } else {
            remaining_args.push(arg.clone());
        }

        if matches!(args.first().map(String::as_str), Some("kill"))
            && index > 0
            && !is_kill_option_with_value(args, index)
            && !matches!(arg.as_str(), "-f" | "--force" | "--signal")
        {
            kill_targets_started = true;
        }
    }

    let flags = GlobalFlags::try_parse_from(flag_args)?;
    parse_strict_clap_contract(binary_name, &remaining_args)?;
    let completion_shell = parse_completion_shell(binary_name, &remaining_args)?;
    validate_query_inputs(&remaining_args)?;

    Ok(ParsedCliArgs {
        binary_name: binary_name.to_string(),
        show_all: flags.show_all,
        quiet: flags.quiet,
        ascii: flags.ascii,
        verbose: flags.verbose,
        json: flags.json,
        completion_shell,
        remaining_args,
    })
}

pub fn command(binary_name: &str) -> Command {
    if binary_name == "whoisonport" {
        return Command::new("whoisonport")
            .disable_help_subcommand(true)
            .disable_version_flag(true)
            .arg(Arg::new("target"));
    }

    Command::new("ports")
        .disable_help_subcommand(true)
        .disable_version_flag(true)
        .arg(
            Arg::new("all")
                .short('a')
                .long("all")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .long("quiet")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("ascii")
                .long("ascii")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .subcommand(Command::new("help"))
        .subcommand(Command::new("ps").args(query_filter_args()))
        .subcommand(
            Command::new("check").arg(
                Arg::new("ports")
                    .required(true)
                    .num_args(1..)
                    .value_parser(clap::value_parser!(u16)),
            ),
        )
        .subcommand(
            Command::new("open").arg(
                Arg::new("port")
                    .required(true)
                    .value_parser(clap::value_parser!(u16)),
            ),
        )
        .subcommand(Command::new("clean"))
        .subcommand(
            Command::new("kill")
                .arg(
                    Arg::new("force")
                        .short('f')
                        .long("force")
                        .action(ArgAction::SetTrue),
                )
                .arg(Arg::new("signal").long("signal").num_args(1))
                .arg(Arg::new("target").num_args(0..).allow_hyphen_values(true)),
        )
        .subcommand(
            Command::new("logs")
                .arg(Arg::new("target"))
                .arg(
                    Arg::new("follow")
                        .short('f')
                        .long("follow")
                        .action(ArgAction::SetTrue),
                )
                .arg(Arg::new("lines").long("lines").num_args(1))
                .arg(Arg::new("grep").long("grep").num_args(1))
                .arg(Arg::new("since").long("since").num_args(1))
                .arg(Arg::new("err").long("err").action(ArgAction::SetTrue)),
        )
        .subcommand(Command::new("watch"))
        .subcommand(
            Command::new("completion").arg(
                Arg::new("shell")
                    .required(true)
                    .value_parser(EnumValueParser::<Shell>::new()),
            ),
        )
        .args(query_filter_args())
        .arg(Arg::new("target").allow_hyphen_values(true))
}

fn query_filter_args() -> Vec<Arg> {
    vec![
        Arg::new("framework").long("framework").num_args(1),
        Arg::new("pid").long("pid").num_args(1),
        Arg::new("project").long("project").num_args(1),
        Arg::new("port-range").long("port-range").num_args(1),
    ]
}

fn parse_completion_shell(
    binary_name: &str,
    args: &[String],
) -> Result<Option<Shell>, clap::Error> {
    if args.first().map(String::as_str) != Some("completion") {
        return Ok(None);
    }

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(None);
    }

    let matches = command(binary_name).try_get_matches_from(
        std::iter::once(binary_name.to_string()).chain(args.iter().cloned()),
    )?;
    let Some((_, subcommand_matches)) = matches.subcommand() else {
        return Ok(Some(Shell::Bash));
    };
    Ok(subcommand_matches.get_one::<Shell>("shell").copied())
}

fn parse_strict_clap_contract(binary_name: &str, args: &[String]) -> Result<(), clap::Error> {
    if should_validate_with_clap(args) {
        command(binary_name).try_get_matches_from(
            std::iter::once(binary_name.to_string()).chain(args.iter().cloned()),
        )?;
    }
    Ok(())
}

fn should_validate_with_clap(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some("help" | "clean" | "watch" | "completion" | "check" | "open") => true,
        Some("ps") => args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h" || !is_query_filter_flag(arg)),
        Some("kill") => args.iter().any(|arg| arg == "--help" || arg == "-h"),
        Some("logs") => true,
        _ => args.iter().any(|arg| arg == "--help" || arg == "-h"),
    }
}

fn is_global_flag(arg: &str) -> bool {
    matches!(
        arg,
        "--all" | "-a" | "--quiet" | "--ascii" | "--verbose" | "--json"
    )
}

fn is_kill_display_target(
    args: &[String],
    index: usize,
    arg: &str,
    kill_targets_started: bool,
) -> bool {
    matches!(arg, "--quiet" | "--ascii")
        && matches!(args.first().map(String::as_str), Some("kill"))
        && index > 0
        && kill_targets_started
}

fn is_kill_option_with_value(args: &[String], index: usize) -> bool {
    matches!(
        args.get(index.wrapping_sub(1)).map(String::as_str),
        Some("--signal")
    )
}

fn validate_query_inputs(args: &[String]) -> Result<(), clap::Error> {
    if !is_query_command_context(args) {
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(());
    }

    let query_args = match args.first().map(String::as_str) {
        Some("ps") => &args[1..],
        _ => args,
    };

    let mut has_positional_range = false;
    let mut has_flag_range = false;
    let mut index = 0;
    while index < query_args.len() {
        let arg = query_args[index].as_str();
        match arg {
            "--framework" | "--pid" | "--project" => {
                let Some(value) = query_args.get(index + 1) else {
                    return Err(invalid_value_error(format!("missing value for {arg}")));
                };
                if arg == "--pid" && value.parse::<u32>().is_err() {
                    return Err(invalid_value_error(format!(
                        "invalid value '{value}' for --pid"
                    )));
                }
                index += 2;
            }
            "--port-range" => {
                let Some(value) = query_args.get(index + 1) else {
                    return Err(invalid_value_error("missing value for --port-range"));
                };
                parse_port_range_value(value)?;
                has_flag_range = true;
                index += 2;
            }
            _ => {
                if arg.contains('-') {
                    parse_port_range_value(arg)?;
                    has_positional_range = true;
                }
                index += 1;
            }
        }
    }

    if has_positional_range && has_flag_range {
        return Err(clap::Error::raw(
            clap::error::ErrorKind::ArgumentConflict,
            "cannot combine a positional port range with --port-range",
        ));
    }

    Ok(())
}

fn is_query_command_context(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        None => true,
        Some(
            "help" | "--help" | "-h" | "clean" | "kill" | "logs" | "watch" | "completion" | "check"
            | "open",
        ) => false,
        Some("ps") => true,
        Some(arg) => {
            is_query_filter_flag(arg) || looks_like_port_range(arg) || arg.parse::<u32>().is_ok()
        }
    }
}

fn is_query_filter_flag(arg: &str) -> bool {
    matches!(arg, "--framework" | "--pid" | "--project" | "--port-range")
}

fn looks_like_port_range(arg: &str) -> bool {
    let Some((start, end)) = arg.split_once('-') else {
        return false;
    };
    !start.is_empty()
        && !end.is_empty()
        && start.parse::<u16>().is_ok()
        && end.parse::<u16>().is_ok()
}

fn parse_port_range_value(value: &str) -> Result<(u16, u16), clap::Error> {
    let Some((start, end)) = value.split_once('-') else {
        return Err(invalid_value_error(format!("invalid port range '{value}'")));
    };
    let start = start
        .parse::<u16>()
        .map_err(|_| invalid_value_error(format!("invalid port range '{value}'")))?;
    let end = end
        .parse::<u16>()
        .map_err(|_| invalid_value_error(format!("invalid port range '{value}'")))?;
    if start > end {
        return Err(invalid_value_error(format!("invalid port range '{value}'")));
    }
    Ok((start, end))
}

fn invalid_value_error(message: impl Into<String>) -> clap::Error {
    clap::Error::raw(clap::error::ErrorKind::ValueValidation, message.into())
}

#[cfg(test)]
mod tests {
    use super::{ParsedCliArgs, parse};
    use clap_complete::Shell;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_global_flags_without_consuming_command_arguments() {
        assert_eq!(
            parse(
                "ports",
                &args(&[
                    "--json",
                    "logs",
                    "3000",
                    "--lines=5",
                    "-f",
                    "--grep",
                    "error",
                    "--since",
                    "5m"
                ])
            )
            .expect("global flags should parse"),
            ParsedCliArgs {
                binary_name: "ports".to_string(),
                show_all: false,
                quiet: false,
                ascii: false,
                verbose: false,
                json: true,
                completion_shell: None,
                remaining_args: args(&[
                    "logs",
                    "3000",
                    "--lines=5",
                    "-f",
                    "--grep",
                    "error",
                    "--since",
                    "5m"
                ]),
            }
        );
    }

    #[test]
    fn preserves_query_filter_flags_and_range_arguments_in_remaining_argv() {
        assert_eq!(
            parse(
                "ports",
                &args(&[
                    "ps",
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
            .expect("query filter flags should remain command-specific"),
            ParsedCliArgs {
                binary_name: "ports".to_string(),
                show_all: false,
                quiet: false,
                ascii: false,
                verbose: false,
                json: false,
                completion_shell: None,
                remaining_args: args(&[
                    "ps",
                    "--framework",
                    "nextjs",
                    "--project",
                    "demo",
                    "--pid",
                    "42",
                    "--port-range",
                    "3000-3010",
                ]),
            }
        );

        assert_eq!(
            parse("ports", &args(&["3000-3010", "--framework", "nextjs"]))
                .expect("positional ranges should stay in remaining args"),
            ParsedCliArgs {
                binary_name: "ports".to_string(),
                show_all: false,
                quiet: false,
                ascii: false,
                verbose: false,
                json: false,
                completion_shell: None,
                remaining_args: args(&["3000-3010", "--framework", "nextjs"]),
            }
        );
    }

    #[test]
    fn parses_quiet_and_ascii_as_shared_global_flags() {
        assert_eq!(
            parse("ports", &args(&["ps", "--quiet", "--ascii"]))
                .expect("shared display flags should parse"),
            ParsedCliArgs {
                binary_name: "ports".to_string(),
                show_all: false,
                quiet: true,
                ascii: true,
                verbose: false,
                json: false,
                completion_shell: None,
                remaining_args: args(&["ps"]),
            }
        );
    }

    #[test]
    fn preserves_hyphenated_kill_targets_that_match_global_flag_names() {
        assert_eq!(
            parse("ports", &args(&["kill", "3000", "--ascii"]))
                .expect("kill targets should be preserved"),
            ParsedCliArgs {
                binary_name: "ports".to_string(),
                show_all: false,
                quiet: false,
                ascii: false,
                verbose: false,
                json: false,
                completion_shell: None,
                remaining_args: args(&["kill", "3000", "--ascii"]),
            }
        );
    }

    #[test]
    fn parses_kill_display_flags_as_global_before_targets_begin() {
        assert_eq!(
            parse("ports", &args(&["kill", "--quiet", "3000", "--ascii"]))
                .expect("leading kill display flags should stay global"),
            ParsedCliArgs {
                binary_name: "ports".to_string(),
                show_all: false,
                quiet: true,
                ascii: false,
                verbose: false,
                json: false,
                completion_shell: None,
                remaining_args: args(&["kill", "3000", "--ascii"]),
            }
        );
    }

    #[test]
    fn parses_completion_shell_from_shared_clap_model() {
        assert_eq!(
            parse("ports", &args(&["completion", "bash"]))
                .expect("completion should parse")
                .completion_shell,
            Some(Shell::Bash)
        );
    }

    #[test]
    fn rejects_invalid_completion_shell_values() {
        let error = parse("ports", &args(&["completion", "invalid-shell"]))
            .expect_err("invalid shell should fail clap validation");

        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn clap_handles_subcommand_help_requests_before_business_validation() {
        for argv in [
            args(&["logs", "--help"]),
            args(&["kill", "--help"]),
            args(&["watch", "--help"]),
            args(&["ps", "--help"]),
            args(&["completion", "--help"]),
        ] {
            let error =
                parse("ports", &argv).expect_err("subcommand help should short-circuit via clap");
            assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
        }
    }

    #[test]
    fn rejects_logs_flags_without_required_values() {
        for argv in [
            args(&["logs", "3000", "--grep"]),
            args(&["logs", "3000", "--since"]),
        ] {
            let error = parse("ports", &argv)
                .expect_err("missing logs option values should fail clap validation");
            assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
        }
    }

    #[test]
    fn preserves_active_binary_name_for_alias_help_output() {
        let error =
            parse("whoisonport", &args(&["--help"])).expect_err("help should be rendered by clap");

        let rendered = error.render().to_string();
        assert!(
            rendered.contains("Usage: whoisonport"),
            "unexpected help: {rendered}"
        );
        assert!(
            !rendered.contains("Usage: ports"),
            "unexpected help: {rendered}"
        );
    }

    #[test]
    fn kill_help_lists_signal_selection_option() {
        let error = parse("ports", &args(&["kill", "--help"]))
            .expect_err("kill help should be rendered by clap");

        let rendered = error.render().to_string();
        assert!(rendered.contains("--signal"), "unexpected help: {rendered}");
    }

    #[test]
    fn rejects_unexpected_extra_arguments_for_strict_commands() {
        for argv in [
            args(&["clean", "unexpected"]),
            args(&["watch", "unexpected"]),
            args(&["help", "unexpected", "extra"]),
            args(&["ps", "unexpected"]),
        ] {
            let error =
                parse("ports", &argv).expect_err("unexpected args should fail clap validation");
            assert!(
                matches!(
                    error.kind(),
                    clap::error::ErrorKind::UnknownArgument | clap::error::ErrorKind::TooManyValues
                ),
                "unexpected error kind: {:?}",
                error.kind()
            );
        }
    }

    #[test]
    fn rejects_invalid_pid_filter_values() {
        let error = parse("ports", &args(&["--pid", "nope"]))
            .expect_err("invalid pid filter should fail fast");

        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn rejects_invalid_port_range_filter_values() {
        let malformed = parse("ports", &args(&["--port-range", "abc"]))
            .expect_err("malformed port range should fail fast");
        assert_eq!(malformed.kind(), clap::error::ErrorKind::ValueValidation);

        let reversed = parse("ports", &args(&["--port-range", "4000-3000"]))
            .expect_err("reversed port range should fail fast");
        assert_eq!(reversed.kind(), clap::error::ErrorKind::ValueValidation);
    }
}
