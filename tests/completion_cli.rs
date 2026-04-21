use std::process::Command;

fn run_completion(shell: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ports"))
        .args(["completion", shell])
        .output()
        .expect("ports binary should run")
}

#[test]
fn emits_bash_completion_script_to_stdout() {
    let output = run_completion("bash");

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(
        stdout.contains("complete -F") && stdout.contains(" ports"),
        "expected bash completion script, got: {stdout}"
    );
}

#[test]
fn emits_zsh_completion_script_to_stdout() {
    let output = run_completion("zsh");

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(
        stdout.contains("#compdef ports") || stdout.contains("compdef _ports ports"),
        "expected zsh completion script, got: {stdout}"
    );
}

#[test]
fn emits_fish_completion_script_to_stdout() {
    let output = run_completion("fish");

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(
        stdout.contains("complete -c ports"),
        "expected fish completion script, got: {stdout}"
    );
}

#[test]
fn fish_completion_does_not_advertise_query_filters_for_unrelated_commands() {
    let output = run_completion("fish");

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");

    for command in ["help", "clean", "kill", "logs", "watch", "completion"] {
        assert!(
            !stdout.contains(&format!(
                "complete -c ports -n \"__fish_ports_using_subcommand {command}\" -l framework -r"
            )),
            "unexpected query filter completion for {command}: {stdout}"
        );
        assert!(
            !stdout.contains(&format!(
                "complete -c ports -n \"__fish_ports_using_subcommand {command}\" -l pid -r"
            )),
            "unexpected query filter completion for {command}: {stdout}"
        );
        assert!(
            !stdout.contains(&format!(
                "complete -c ports -n \"__fish_ports_using_subcommand {command}\" -l project -r"
            )),
            "unexpected query filter completion for {command}: {stdout}"
        );
        assert!(
            !stdout.contains(&format!(
                "complete -c ports -n \"__fish_ports_using_subcommand {command}\" -l port-range -r"
            )),
            "unexpected query filter completion for {command}: {stdout}"
        );
    }
}

#[test]
fn invalid_completion_shell_reports_completion_parse_error() {
    let output = run_completion("nope");

    assert!(
        !output.status.success(),
        "expected failure, got: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(
        !stdout.contains("Unknown command: completion"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        stderr.contains("<shell>"),
        "expected shell parse error, got: {stderr}"
    );
    assert!(
        stderr.contains("possible values") || stderr.contains("invalid value"),
        "expected clap shell validation error, got: {stderr}"
    );
}
