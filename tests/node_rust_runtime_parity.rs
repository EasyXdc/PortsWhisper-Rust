use port_whisperer::test_support::{ChildProcessHarness, allocate_unused_tcp_port};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn node_and_rust_both_detect_controlled_local_server_port() {
    let port = allocate_unused_tcp_port().expect("port should be allocated");
    let mut harness =
        ChildProcessHarness::spawn_node_http_server(port).expect("node fixture should start");

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let node_output = Command::new("node")
        .args(["src/index.js", &port.to_string()])
        .current_dir("/Users/easyxdc/Desktop/PersonalDocument/MyCode/2026/port-whisperer")
        .output()
        .expect("node reference should run");
    assert!(node_output.status.success());
    let node_stdout = String::from_utf8_lossy(&node_output.stdout);
    assert!(node_stdout.contains(&format!(":{port}")));

    let rust_output = Command::new("cargo")
        .args(["run", "--quiet", "--bin", "ports", "--", &port.to_string()])
        .current_dir("/Users/easyxdc/Desktop/PersonalDocument/MyCode/2026/port-whisperer-rust")
        .output()
        .expect("rust cli should run");
    assert!(
        rust_output.status.success(),
        "rust stderr: {}",
        String::from_utf8_lossy(&rust_output.stderr)
    );
    let rust_stdout = String::from_utf8_lossy(&rust_output.stdout);
    assert!(rust_stdout.contains(&format!(":{port}")));

    harness.kill().expect("fixture should be cleaned up");
}
