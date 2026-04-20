use crate::model::{
    DockerInfo, LogFile, ProcessTreeNode, RawPortEntry, RawProcessDetails, RawProcessEntry,
};
use crate::platform::PlatformScanner;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Default)]
pub(crate) struct FakePlatformScanner {
    pub listening_ports: Vec<RawPortEntry>,
    pub process_details: HashMap<u32, RawProcessDetails>,
    pub cwd: HashMap<u32, PathBuf>,
    pub all_processes: Vec<RawProcessEntry>,
    pub process_trees: HashMap<u32, Vec<ProcessTreeNode>>,
    pub existing_pids: HashSet<u32>,
    pub log_files: HashMap<u32, Vec<LogFile>>,
    pub system_log_command: Option<String>,
}

impl PlatformScanner for FakePlatformScanner {
    fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
        self.listening_ports.clone()
    }

    fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
        pids.iter()
            .filter_map(|pid| {
                self.process_details
                    .get(pid)
                    .map(|details| (*pid, details.clone()))
            })
            .collect()
    }

    fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf> {
        pids.iter()
            .filter_map(|pid| self.cwd.get(pid).map(|cwd| (*pid, cwd.clone())))
            .collect()
    }

    fn get_all_processes_raw(&self) -> Vec<RawProcessEntry> {
        self.all_processes.clone()
    }

    fn get_process_tree(&self, pid: u32) -> Vec<ProcessTreeNode> {
        self.process_trees.get(&pid).cloned().unwrap_or_default()
    }

    fn pid_exists(&self, pid: u32) -> bool {
        self.existing_pids.contains(&pid)
    }

    fn kill_process(&self, _pid: u32, _signal: &str) -> bool {
        true
    }

    fn get_process_log_files(&self, pid: u32) -> Vec<LogFile> {
        self.log_files.get(&pid).cloned().unwrap_or_default()
    }

    fn get_system_log_command(&self, _pid: u32, _follow: bool) -> Option<String> {
        self.system_log_command.clone()
    }
}

pub fn allocate_unused_tcp_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

pub struct ChildProcessHarness {
    child: Child,
    stdout_path: Option<PathBuf>,
    stderr_path: Option<PathBuf>,
}

impl ChildProcessHarness {
    pub(crate) fn spawn_sleep() -> std::io::Result<Self> {
        let child = if cfg!(target_os = "windows") {
            Command::new("powershell")
                .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?
        } else {
            Command::new("sleep")
                .arg("30")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?
        };
        Ok(Self {
            child,
            stdout_path: None,
            stderr_path: None,
        })
    }

    pub(crate) fn spawn_generic_long_running_process() -> std::io::Result<Self> {
        Self::spawn_sleep()
    }

    pub fn spawn_node_http_server(port: u16) -> std::io::Result<Self> {
        let script = format!(
            "const http=require('http'); const server=http.createServer((_,res)=>res.end('ok')); server.listen({}, '127.0.0.1'); setInterval(()=>{{}}, 1000);",
            port
        );
        let child = Command::new("node")
            .args(["-e", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self {
            child,
            stdout_path: None,
            stderr_path: None,
        })
    }

    pub(crate) fn spawn_python_http_server(port: u16) -> std::io::Result<Self> {
        let child = Command::new("python3")
            .args([
                "-m",
                "http.server",
                &port.to_string(),
                "--bind",
                "127.0.0.1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self {
            child,
            stdout_path: None,
            stderr_path: None,
        })
    }

    pub(crate) fn spawn_stdout_redirect_process(message: &str) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "port-whisperer-stdout-{}-{}.log",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let stdout = File::create(&path)?;
        let script = format!("console.log({:?}); setInterval(() => {{}}, 1000);", message);
        let child = Command::new("node")
            .args(["-e", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self {
            child,
            stdout_path: Some(path),
            stderr_path: None,
        })
    }

    pub(crate) fn spawn_stderr_redirect_process(message: &str) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "port-whisperer-stderr-{}-{}.log",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let stderr = File::create(&path)?;
        let script = format!(
            "console.error({:?}); setInterval(() => {{}}, 1000);",
            message
        );
        let child = Command::new("node")
            .args(["-e", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr))
            .spawn()?;
        Ok(Self {
            child,
            stdout_path: None,
            stderr_path: Some(path),
        })
    }

    pub(crate) fn spawn_no_redirection_process() -> std::io::Result<Self> {
        let child = Command::new("node")
            .args(["-e", "setInterval(() => {}, 1000);"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self {
            child,
            stdout_path: None,
            stderr_path: None,
        })
    }

    pub(crate) fn pid(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn stdout_path(&self) -> Option<&Path> {
        self.stdout_path.as_deref()
    }

    pub(crate) fn stderr_path(&self) -> Option<&Path> {
        self.stderr_path.as_deref()
    }

    pub(crate) fn is_running(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    pub fn kill(&mut self) -> std::io::Result<()> {
        match self.child.try_wait()? {
            Some(_) => Ok(()),
            None => {
                self.child.kill()?;
                let _ = self.child.wait();
                Ok(())
            }
        }
    }
}

impl Drop for ChildProcessHarness {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

pub(crate) fn create_vite_like_fixture() -> std::io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "port-whisperer-vite-fixture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root)?;
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"vite":"latest"}}"#,
    )?;
    Ok(root)
}

pub(crate) fn create_next_like_fixture() -> std::io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "port-whisperer-next-fixture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root)?;
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"next":"latest"}}"#,
    )?;
    std::fs::write(root.join("next.config.js"), "module.exports = {}\n")?;
    Ok(root)
}

pub(crate) fn create_express_like_fixture() -> std::io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "port-whisperer-express-fixture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root)?;
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"express":"latest"}}"#,
    )?;
    Ok(root)
}

pub(crate) struct CommandFixture {
    pub process_name: String,
    pub command: String,
}

pub(crate) fn create_fastapi_like_command_fixture() -> CommandFixture {
    CommandFixture {
        process_name: "python3".to_string(),
        command: "python -m uvicorn app:app --host 127.0.0.1 --port 8000".to_string(),
    }
}

pub(crate) fn create_localstack_like_mapping(port: u16) -> HashMap<u16, DockerInfo> {
    HashMap::from([(
        port,
        DockerInfo {
            host_port: port,
            container_name: "localstack-main".to_string(),
            image: "localstack/localstack:latest".to_string(),
            framework: "LocalStack".to_string(),
        },
    )])
}

pub(crate) fn docker_available() -> bool {
    Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(crate) struct DockerFixtureGuard {
    skipped: bool,
    reason: Option<String>,
    container_name: Option<String>,
}

impl DockerFixtureGuard {
    pub(crate) fn from_probe(available: bool, reason: &str) -> Self {
        if available {
            Self {
                skipped: false,
                reason: None,
                container_name: None,
            }
        } else {
            Self {
                skipped: true,
                reason: Some(reason.to_string()),
                container_name: None,
            }
        }
    }

    pub(crate) fn start_nginx(port: u16) -> std::io::Result<Self> {
        let container_name = format!(
            "port-whisperer-nginx-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let status = Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "--name",
                &container_name,
                "-p",
                &format!("{}:80", port),
                "nginx:alpine",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(std::io::Error::other("failed to start nginx docker fixture"));
        }

        Ok(Self {
            skipped: false,
            reason: None,
            container_name: Some(container_name),
        })
    }

    pub(crate) fn start_redis(port: u16) -> std::io::Result<Self> {
        let container_name = format!(
            "port-whisperer-redis-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let status = Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "--name",
                &container_name,
                "-p",
                &format!("{}:6379", port),
                "redis:alpine",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(std::io::Error::other("failed to start redis docker fixture"));
        }

        Ok(Self {
            skipped: false,
            reason: None,
            container_name: Some(container_name),
        })
    }

    pub(crate) fn start_postgres(port: u16) -> std::io::Result<Self> {
        let container_name = format!(
            "port-whisperer-postgres-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let status = Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "--name",
                &container_name,
                "-e",
                "POSTGRES_PASSWORD=postgres",
                "-p",
                &format!("{}:5432", port),
                "postgres:16-alpine",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(std::io::Error::other("failed to start postgres docker fixture"));
        }

        Ok(Self {
            skipped: false,
            reason: None,
            container_name: Some(container_name),
        })
    }

    pub(crate) fn is_skipped(&self) -> bool {
        self.skipped
    }

    pub(crate) fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    pub(crate) fn cleanup(&mut self) -> std::io::Result<()> {
        if let Some(name) = self.container_name.take() {
            let _ = Command::new("docker")
                .args(["rm", "-f", &name])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
        }
        Ok(())
    }
}

impl Drop for DockerFixtureGuard {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChildProcessHarness, DockerFixtureGuard, allocate_unused_tcp_port,
        create_express_like_fixture, create_fastapi_like_command_fixture,
        create_localstack_like_mapping, create_next_like_fixture, create_vite_like_fixture,
        docker_available,
    };
    use crate::framework::{detect_framework, detect_framework_from_command};
    use std::fs;
    use std::net::TcpStream;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn test_harness_tracks_child_pid_and_cleans_up() {
        let port = allocate_unused_tcp_port().expect("port should be allocated");
        assert!(port > 0);

        let mut harness = ChildProcessHarness::spawn_sleep().expect("child should start");
        let pid = harness.pid();
        assert!(pid > 0);
        assert!(harness.is_running());

        harness.kill().expect("child should be killed");
    }

    #[test]
    fn node_http_fixture_listens_on_requested_random_port() {
        let port = allocate_unused_tcp_port().expect("port should be allocated");
        let mut harness =
            ChildProcessHarness::spawn_node_http_server(port).expect("node server should start");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut connected = false;
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                connected = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert!(connected, "node server should accept tcp connections");
        harness.kill().expect("node server should be killed");
    }

    #[test]
    fn stdout_redirect_fixture_writes_to_temp_file() {
        let mut harness = ChildProcessHarness::spawn_stdout_redirect_process("hello from fixture")
            .expect("stdout redirect fixture should start");

        let stdout_path = harness.stdout_path().expect("stdout file should exist");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut contents = String::new();
        while Instant::now() < deadline {
            contents = fs::read_to_string(stdout_path).unwrap_or_default();
            if contents.contains("hello from fixture") {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert!(contents.contains("hello from fixture"));
        harness.kill().expect("fixture should be killed");
    }

    #[test]
    fn stderr_redirect_fixture_writes_to_temp_file() {
        let mut harness = ChildProcessHarness::spawn_stderr_redirect_process("hello from stderr")
            .expect("stderr redirect fixture should start");

        let stderr_path = harness.stderr_path().expect("stderr file should exist");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut contents = String::new();
        while Instant::now() < deadline {
            contents = fs::read_to_string(stderr_path).unwrap_or_default();
            if contents.contains("hello from stderr") {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert!(contents.contains("hello from stderr"));
        harness.kill().expect("fixture should be killed");
    }

    #[test]
    fn no_redirection_fixture_exposes_no_log_files() {
        let mut harness = ChildProcessHarness::spawn_no_redirection_process()
            .expect("no-redirection fixture should start");

        assert!(harness.is_running());
        assert!(harness.stdout_path().is_none());
        assert!(harness.stderr_path().is_none());

        harness.kill().expect("fixture should be killed");
    }

    #[test]
    fn python_http_fixture_listens_on_requested_random_port() {
        let port = allocate_unused_tcp_port().expect("port should be allocated");
        let mut harness = ChildProcessHarness::spawn_python_http_server(port)
            .expect("python server should start");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut connected = false;
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                connected = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert!(connected, "python server should accept tcp connections");
        harness.kill().expect("python server should be killed");
    }

    #[test]
    fn vite_like_package_fixture_is_detected_as_vite() {
        let fixture = create_vite_like_fixture().expect("vite fixture should be created");

        assert_eq!(detect_framework(&fixture).as_deref(), Some("Vite"));
        assert!(fixture.join("package.json").exists());

        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn next_like_package_fixture_is_detected_as_nextjs() {
        let fixture = create_next_like_fixture().expect("next fixture should be created");

        assert_eq!(detect_framework(&fixture).as_deref(), Some("Next.js"));
        assert!(fixture.join("package.json").exists());

        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn express_like_package_fixture_is_detected_as_express() {
        let fixture = create_express_like_fixture().expect("express fixture should be created");

        assert_eq!(detect_framework(&fixture).as_deref(), Some("Express"));
        assert!(fixture.join("package.json").exists());

        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn fastapi_like_command_fixture_is_detected_as_fastapi() {
        let fixture = create_fastapi_like_command_fixture();

        assert_eq!(
            detect_framework_from_command(&fixture.command, &fixture.process_name).as_deref(),
            Some("FastAPI")
        );
    }

    #[test]
    fn localstack_like_mapping_marks_framework_as_localstack() {
        let mapping = create_localstack_like_mapping(4566);
        let info = mapping.get(&4566).expect("mapping should contain port");

        assert_eq!(info.container_name, "localstack-main");
        assert_eq!(info.image, "localstack/localstack:latest");
        assert_eq!(info.framework, "LocalStack");
    }

    #[test]
    fn generic_long_running_fixture_stays_alive_until_cleaned_up() {
        let mut harness = ChildProcessHarness::spawn_generic_long_running_process()
            .expect("generic long-running fixture should start");

        assert!(harness.pid() > 0);
        assert!(harness.is_running());

        harness.kill().expect("fixture should be killed");
    }

    #[test]
    fn docker_fixture_guard_skips_cleanly_when_docker_unavailable() {
        let unavailable = DockerFixtureGuard::from_probe(false, "docker test");
        assert!(unavailable.is_skipped());
        assert_eq!(unavailable.reason(), Some("docker test"));
    }

    #[test]
    fn docker_availability_probe_is_safe_to_call() {
        let _ = docker_available();
    }

    #[test]
    fn nginx_docker_fixture_binds_host_port_when_docker_available() {
        if !docker_available() {
            return;
        }

        let port = allocate_unused_tcp_port().expect("port should be allocated");
        let mut guard = DockerFixtureGuard::start_nginx(port).expect("nginx fixture should start");

        let deadline = Instant::now() + Duration::from_secs(15);
        let mut connected = false;
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                connected = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        assert!(connected, "nginx container should accept tcp connections");
        guard.cleanup().expect("nginx fixture should be cleaned up");
    }

    #[test]
    fn redis_docker_fixture_binds_host_port_when_docker_available() {
        if !docker_available() {
            return;
        }

        let port = allocate_unused_tcp_port().expect("port should be allocated");
        let mut guard = DockerFixtureGuard::start_redis(port).expect("redis fixture should start");

        let deadline = Instant::now() + Duration::from_secs(15);
        let mut connected = false;
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                connected = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        assert!(connected, "redis container should accept tcp connections");
        guard.cleanup().expect("redis fixture should be cleaned up");
    }

    #[test]
    fn postgres_docker_fixture_binds_host_port_when_docker_available() {
        if !docker_available() {
            return;
        }

        let port = allocate_unused_tcp_port().expect("port should be allocated");
        let mut guard =
            DockerFixtureGuard::start_postgres(port).expect("postgres fixture should start");

        let deadline = Instant::now() + Duration::from_secs(20);
        let mut connected = false;
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                connected = true;
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }

        assert!(
            connected,
            "postgres container should accept tcp connections"
        );
        guard
            .cleanup()
            .expect("postgres fixture should be cleaned up");
    }
}
