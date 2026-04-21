use std::process::Command;

fn run_ports(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ports"))
        .args(args)
        .output()
        .expect("ports binary should run")
}

#[test]
fn open_requires_a_port_argument() {
    let output = run_ports(&["open"]);

    assert!(
        !output.status.success(),
        "expected failure, got: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(stdout.trim().is_empty(), "unexpected stdout: {stdout}");
    assert!(
        stderr.contains("Usage: ports open") || stderr.contains("required"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn open_rejects_non_numeric_port_arguments() {
    let output = run_ports(&["open", "nope"]);

    assert!(
        !output.status.success(),
        "expected failure, got: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(stdout.trim().is_empty(), "unexpected stdout: {stdout}");
    assert!(
        stderr.contains("invalid")
            || stderr.contains("value")
            || stderr.contains("Usage: ports open"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn open_rejects_out_of_range_port_arguments() {
    let output = run_ports(&["open", "70000"]);

    assert!(
        !output.status.success(),
        "expected failure, got: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(stdout.trim().is_empty(), "unexpected stdout: {stdout}");
    assert!(
        stderr.contains("invalid")
            || stderr.contains("value")
            || stderr.contains("Usage: ports open"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn open_rejects_json_output() {
    let output = run_ports(&["--json", "open", "3000"]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected failure, got: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(stdout.trim().is_empty(), "unexpected stdout: {stdout}");
    assert!(
        stderr.contains("JSON output is not supported for 'open' yet."),
        "unexpected stderr: {stderr}"
    );
}
