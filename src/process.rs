use crate::framework::{
    detect_framework, detect_framework_from_command, is_dev_process, is_docker_process,
    summarize_command,
};
use crate::model::{
    KillResolutionKind, KillTargetResolution, PortInfo, ProcessInfo, ProcessStatus,
};
use crate::platform::{self, PlatformScanner};
use crate::ports;
use crate::util::{find_project_root, format_memory, format_uptime_from_lstart, path_basename};

pub fn get_all_processes() -> Vec<ProcessInfo> {
    get_all_processes_with(platform::native_scanner())
}

pub(crate) fn get_all_processes_with(scanner: &dyn PlatformScanner) -> Vec<ProcessInfo> {
    let entries = scanner.get_all_processes_raw();
    let non_docker_pids: Vec<u32> = entries
        .iter()
        .filter(|e| !is_docker_process(&e.process_name))
        .map(|e| e.pid)
        .collect();
    let cwd_map = scanner.batch_cwd(&non_docker_pids);
    entries
        .into_iter()
        .map(|e| {
            let cwd = cwd_map.get(&e.pid);
            let mut info = ProcessInfo {
                pid: e.pid,
                ppid: None,
                process_name: e.process_name.clone(),
                command: e.command.clone(),
                description: summarize_command(&e.command, &e.process_name),
                cpu: e.cpu,
                rss_kb: e.rss_kb,
                memory: (e.rss_kb > 0).then(|| format_memory(e.rss_kb)),
                cwd: None,
                project_name: None,
                framework: detect_framework_from_command(&e.command, &e.process_name),
                uptime: e.lstart.as_deref().and_then(format_uptime_from_lstart),
                status_raw: String::new(),
            };
            if let Some(cwd) = cwd {
                let root = find_project_root(cwd);
                info.cwd = Some(root.clone());
                info.project_name = path_basename(&root);
                if info.framework.is_none() {
                    info.framework = detect_framework(&root);
                }
            }
            info
        })
        .collect()
}

pub fn find_orphaned_processes() -> Vec<PortInfo> {
    ports::get_listening_ports(false)
        .into_iter()
        .filter(|p| matches!(p.status, ProcessStatus::Orphaned | ProcessStatus::Zombie))
        .collect()
}

pub fn resolve_kill_target(n: u32) -> Option<KillTargetResolution> {
    resolve_kill_target_with(n, ports::get_port_details, platform::pid_exists)
}

pub(crate) fn resolve_kill_target_with<PortLookup, PidExists>(
    n: u32,
    port_lookup: PortLookup,
    pid_exists: PidExists,
) -> Option<KillTargetResolution>
where
    PortLookup: Fn(u16) -> Option<PortInfo>,
    PidExists: Fn(u32) -> bool,
{
    if n == 0 {
        return None;
    }
    if n <= 65_535 {
        if let Some(info) = port_lookup(n as u16) {
            return Some(KillTargetResolution {
                pid: info.pid,
                via: KillResolutionKind::Port,
                port: Some(n as u16),
                info: Some(info),
            });
        }
    }
    if pid_exists(n) {
        return Some(KillTargetResolution {
            pid: n,
            via: KillResolutionKind::Pid,
            port: None,
            info: None,
        });
    }
    None
}

pub fn keep_dev_process(info: &ProcessInfo) -> bool {
    is_dev_process(&info.process_name, &info.command)
}

#[cfg(test)]
mod tests {
    use super::{get_all_processes_with, keep_dev_process, resolve_kill_target_with};
    use crate::model::{KillResolutionKind, PortInfo, ProcessStatus, RawProcessEntry};
    use crate::test_support::FakePlatformScanner;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn kill_target_resolution_prefers_ports_then_falls_back_to_pid() {
        let port_info = fake_port(3000, 4242);
        let by_port = resolve_kill_target_with(
            3000,
            |port| (port == 3000).then(|| port_info.clone()),
            |_| true,
        )
        .expect("port should resolve");
        assert_eq!(by_port.pid, 4242);
        assert_eq!(by_port.via, KillResolutionKind::Port);
        assert_eq!(by_port.port, Some(3000));
        assert!(by_port.info.is_some());

        let by_pid = resolve_kill_target_with(3001, |_| None, |pid| pid == 3001)
            .expect("pid should resolve");
        assert_eq!(by_pid.pid, 3001);
        assert_eq!(by_pid.via, KillResolutionKind::Pid);
        assert_eq!(by_pid.port, None);
        assert!(by_pid.info.is_none());

        let large_pid = resolve_kill_target_with(70_000, |_| panic!("not a port"), |_| true)
            .expect("large pid should resolve");
        assert_eq!(large_pid.via, KillResolutionKind::Pid);

        assert!(resolve_kill_target_with(0, |_| None, |_| true).is_none());
        assert!(resolve_kill_target_with(3002, |_| None, |_| false).is_none());
    }

    #[test]
    fn fake_platform_enriches_process_snapshot_and_dev_filtering() {
        let project = temp_project("process-fake");
        fs::write(
            project.join("package.json"),
            r#"{"dependencies":{"vite":"latest"}}"#,
        )
        .unwrap();
        let mut fake = FakePlatformScanner {
            all_processes: vec![
                RawProcessEntry {
                    pid: 42,
                    process_name: "node".to_string(),
                    cpu: 6.5,
                    mem_percent: 0.1,
                    rss_kb: 2048,
                    lstart: Some("Jan 01 00:00:00 2000".to_string()),
                    command: "node /repo/server.js --port 3000".to_string(),
                },
                RawProcessEntry {
                    pid: 50,
                    process_name: "Spotify".to_string(),
                    cpu: 1.0,
                    mem_percent: 0.1,
                    rss_kb: 1024,
                    lstart: None,
                    command: "Spotify".to_string(),
                },
            ],
            ..Default::default()
        };
        fake.cwd.insert(42, project.clone());

        let processes = get_all_processes_with(&fake);
        assert_eq!(processes.len(), 2);
        let expected_project = project_name(&project);
        let node = processes.iter().find(|p| p.pid == 42).unwrap();
        assert_eq!(node.memory.as_deref(), Some("2.0 MB"));
        assert_eq!(
            node.project_name.as_deref(),
            Some(expected_project.as_str())
        );
        assert_eq!(node.framework.as_deref(), Some("Node.js"));
        assert_eq!(node.description, "server.js 3000");
        assert!(keep_dev_process(node));

        let default_processes: Vec<_> = processes
            .iter()
            .filter(|p| keep_dev_process(p))
            .map(|p| p.pid)
            .collect();
        assert_eq!(default_processes, vec![42]);

        fs::remove_dir_all(project).unwrap();
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
            status: ProcessStatus::Healthy,
            memory: None,
            git_branch: None,
            process_tree: Vec::new(),
        }
    }

    fn temp_project(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "port-whisperer-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn project_name(path: &std::path::Path) -> String {
        path.file_name().unwrap().to_string_lossy().to_string()
    }
}
