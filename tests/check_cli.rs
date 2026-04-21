use serde_json::Value;
use std::process::Command;

fn run_ports(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ports"))
        .args(args)
        .output()
        .expect("ports binary should run")
}

#[test]
fn check_reports_available_ports_and_exits_zero() {
    let output = run_ports(&["check", "65001", "65002"]);

    assert!(output.status.success(), "expected success, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(stdout.contains("65001"), "unexpected stdout: {stdout}");
    assert!(stdout.contains("65002"), "unexpected stdout: {stdout}");
    assert!(stdout.contains("available"), "unexpected stdout: {stdout}");
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");
}

#[test]
fn check_reports_occupied_ports_and_exits_non_zero() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let port = listener.local_addr().expect("listener addr should resolve").port();

    let output = run_ports(&["check", &port.to_string()]);

    assert_eq!(output.status.code(), Some(1), "expected occupied ports to exit 1, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(stdout.contains(&port.to_string()), "unexpected stdout: {stdout}");
    assert!(stdout.contains("occupied"), "unexpected stdout: {stdout}");
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");
}

#[test]
fn check_json_reports_port_states_and_exit_code() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let occupied_port = listener.local_addr().expect("listener addr should resolve").port();
    let available_port = 65003_u16;

    let output = run_ports(&[
        "--json",
        "check",
        &occupied_port.to_string(),
        &available_port.to_string(),
    ]);

    assert_eq!(output.status.code(), Some(1), "expected occupied ports to exit 1, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");

    let payload: Value = serde_json::from_str(&stdout).expect("stdout should be valid json");
    assert_eq!(payload["command"], "ports check");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["error"], Value::Null);

    let checks = payload["data"]["ports"].as_array().expect("ports should be an array");
    assert_eq!(checks.len(), 2, "unexpected payload: {payload}");

    assert!(checks.iter().any(|entry| {
        entry["port"] == occupied_port && entry["occupied"] == true && entry["available"] == false
    }), "missing occupied port in payload: {payload}");
    assert!(checks.iter().any(|entry| {
        entry["port"] == available_port && entry["occupied"] == false && entry["available"] == true
    }), "missing available port in payload: {payload}");
}

#[test]
fn check_requires_at_least_one_port_argument() {
    let output = run_ports(&["check"]);

    assert!(!output.status.success(), "expected failure, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(stdout.trim().is_empty(), "unexpected stdout: {stdout}");
    assert!(stderr.contains("Usage: ports check") || stderr.contains("required"), "unexpected stderr: {stderr}");
}

#[test]
fn check_rejects_non_numeric_port_arguments() {
    let output = run_ports(&["check", "nope"]);

    assert!(!output.status.success(), "expected failure, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(stdout.trim().is_empty(), "unexpected stdout: {stdout}");
    assert!(
        stderr.contains("invalid") || stderr.contains("value") || stderr.contains("Usage: ports check"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn check_rejects_out_of_range_port_arguments() {
    let output = run_ports(&["check", "70000"]);

    assert!(!output.status.success(), "expected failure, got: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    assert!(stdout.trim().is_empty(), "unexpected stdout: {stdout}");
    assert!(
        stderr.contains("invalid") || stderr.contains("value") || stderr.contains("Usage: ports check"),
        "unexpected stderr: {stderr}"
    );
}
