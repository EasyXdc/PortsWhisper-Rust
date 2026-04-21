use std::process::Command;

fn run_ports(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ports"))
        .args(args)
        .output()
        .expect("ports binary should run")
}

#[test]
fn logs_help_uses_clap_managed_help_output() {
    let output = run_ports(&["logs", "--help"]);

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(
        stdout.contains("Usage: ports logs"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        !stderr.contains("not a valid port/PID"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn kill_help_uses_clap_managed_help_output() {
    let output = run_ports(&["kill", "--help"]);

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(
        stdout.contains("Usage: ports kill"),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn watch_help_uses_clap_managed_help_output() {
    let output = run_ports(&["watch", "--help"]);

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(
        stdout.contains("Usage: ports watch"),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn ps_help_uses_clap_managed_help_output() {
    let output = run_ports(&["ps", "--help"]);

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(
        stdout.contains("Usage: ports ps"),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn completion_help_uses_clap_managed_help_output() {
    let output = run_ports(&["completion", "--help"]);

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(
        stdout.contains("Usage: ports completion"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        !stderr.contains("unexpected argument '--help'"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn check_help_uses_clap_managed_help_output() {
    let output = run_ports(&["check", "--help"]);

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(
        stdout.contains("Usage: ports check"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        !stderr.contains("unexpected argument '--help'"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn open_help_uses_clap_managed_help_output() {
    let output = run_ports(&["open", "--help"]);

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(
        stdout.contains("Usage: ports open"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        !stderr.contains("unexpected argument '--help'"),
        "unexpected stderr: {stderr}"
    );
}
