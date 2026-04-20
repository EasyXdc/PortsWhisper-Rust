use std::process::Command;

fn run_bin(bin: &str, args: &[&str]) -> std::process::Output {
    let exe = match bin {
        "ports" => env!("CARGO_BIN_EXE_ports"),
        "whoisonport" => env!("CARGO_BIN_EXE_whoisonport"),
        other => panic!("unexpected binary: {other}"),
    };

    Command::new(exe)
        .args(args)
        .output()
        .expect("binary should run")
}

#[test]
fn whoisonport_help_uses_alias_entrypoint_name() {
    let output = run_bin("whoisonport", &["--help"]);

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");

    assert!(stdout.contains("Usage: whoisonport"), "unexpected stdout: {stdout}");
    assert!(!stdout.contains("Usage: ports"), "unexpected stdout: {stdout}");
    assert!(!stdout.contains("ps"), "unexpected stdout: {stdout}");
    assert!(!stdout.contains("clean"), "unexpected stdout: {stdout}");
    assert!(!stdout.contains("kill"), "unexpected stdout: {stdout}");
    assert!(!stdout.contains("logs"), "unexpected stdout: {stdout}");
    assert!(!stdout.contains("watch"), "unexpected stdout: {stdout}");
    assert!(!stdout.contains("completion"), "unexpected stdout: {stdout}");
}

#[test]
fn whoisonport_rejects_ports_subcommands() {
    for argv in [
        vec!["ps"],
        vec!["clean"],
        vec!["kill", "3000"],
        vec!["logs", "3000"],
        vec!["watch"],
        vec!["completion", "bash"],
    ] {
        let output = run_bin("whoisonport", &argv);

        assert!(!output.status.success(), "expected failure for {:?}, got: {output:?}", argv);
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

        assert!(!stdout.contains("Usage: ports"), "unexpected stdout: {stdout}");
        assert!(
            stdout.contains("Unknown command") || stderr.contains("whoisonport") || stderr.contains("port") || stderr.contains("Usage:"),
            "unexpected output stdout={stdout:?} stderr={stderr:?}"
        );
    }
}

#[test]
fn whoisonport_rejects_query_filter_flags() {
    for argv in [
        vec!["3000", "--framework", "nextjs"],
        vec!["3000", "--project", "demo"],
        vec!["3000", "--pid", "42"],
    ] {
        let output = run_bin("whoisonport", &argv);

        assert!(!output.status.success(), "expected failure for {:?}, got: {output:?}", argv);
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

        assert!(!stdout.contains("No process found on that port"), "unexpected stdout: {stdout}");
        assert!(
            stdout.contains("Unknown command") || stderr.contains("whoisonport") || stderr.contains("Usage:") || stderr.contains("unexpected"),
            "unexpected output stdout={stdout:?} stderr={stderr:?}"
        );
    }
}

#[test]
fn whoisonport_rejects_port_ranges() {
    let output = run_bin("whoisonport", &["3000-3010"]);

    assert!(!output.status.success(), "expected failure, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(!stdout.contains("Show dev server ports"), "unexpected stdout: {stdout}");
    assert!(!stdout.contains("No process found on that port"), "unexpected stdout: {stdout}");
    assert!(
        stdout.contains("Unknown command") || stderr.contains("whoisonport") || stderr.contains("Usage:") || stderr.contains("unexpected"),
        "unexpected output stdout={stdout:?} stderr={stderr:?}"
    );
}

#[test]
fn clean_rejects_unexpected_extra_arguments() {
    let output = run_bin("ports", &["clean", "unexpected"]);

    assert!(!output.status.success(), "expected failure, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(!stdout.contains("Found"), "unexpected stdout: {stdout}");
    assert!(stderr.contains("unexpected") || stderr.contains("Usage:"), "unexpected stderr: {stderr}");
}

#[test]
fn watch_rejects_unexpected_extra_arguments() {
    let output = run_bin("ports", &["watch", "unexpected"]);

    assert!(!output.status.success(), "expected failure, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(!stdout.contains("Watching for port changes"), "unexpected stdout: {stdout}");
    assert!(stderr.contains("unexpected") || stderr.contains("Usage:"), "unexpected stderr: {stderr}");
}

#[test]
fn help_rejects_unexpected_extra_arguments() {
    let output = run_bin("ports", &["help", "unexpected"]);

    assert!(!output.status.success(), "expected failure, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(!stdout.contains("Port Whisperer"), "unexpected stdout: {stdout}");
    assert!(stderr.contains("unexpected") || stderr.contains("Usage:"), "unexpected stderr: {stderr}");
}

#[test]
fn ps_rejects_unexpected_extra_arguments() {
    let output = run_bin("ports", &["ps", "unexpected"]);

    assert!(!output.status.success(), "expected failure, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(!stdout.contains("PROCESS"), "unexpected stdout: {stdout}");
    assert!(stderr.contains("unexpected") || stderr.contains("Usage:"), "unexpected stderr: {stderr}");
}
