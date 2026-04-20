use crate::docker;
use crate::framework::{detect_framework, detect_framework_from_command, is_dev_process};
use crate::model::{DisplayTime, DockerInfo, PortInfo, ProcessStatus, RawPortEntry};
use crate::platform::{self, PlatformScanner};
use crate::util::{
    find_project_root, format_memory, format_uptime_from_lstart, path_basename, run_output,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::thread;

pub fn get_listening_ports(detailed: bool) -> Vec<PortInfo> {
    get_listening_ports_with(platform::native_scanner(), detailed, None)
}

pub fn get_port_details(port: u16) -> Option<PortInfo> {
    get_port_details_with(platform::native_scanner(), port, None)
}

pub(crate) fn get_listening_ports_with(
    scanner: &dyn PlatformScanner,
    detailed: bool,
    docker_map_override: Option<HashMap<u16, DockerInfo>>,
) -> Vec<PortInfo> {
    let entries = scanner.get_listening_ports_raw();
    enrich_port_entries(scanner, entries, detailed, docker_map_override)
}

pub(crate) fn get_listening_ports_from_entries(
    scanner: &dyn PlatformScanner,
    entries: Vec<RawPortEntry>,
    detailed: bool,
    docker_map_override: Option<HashMap<u16, DockerInfo>>,
) -> Vec<PortInfo> {
    enrich_port_entries(scanner, entries, detailed, docker_map_override)
}

pub(crate) fn get_port_details_with(
    scanner: &dyn PlatformScanner,
    port: u16,
    docker_map_override: Option<HashMap<u16, DockerInfo>>,
) -> Option<PortInfo> {
    let entry = scanner.get_listening_port_raw(port)?;
    enrich_port_entries(scanner, vec![entry], true, docker_map_override)
        .into_iter()
        .next()
}

fn enrich_port_entries(
    scanner: &dyn PlatformScanner,
    entries: Vec<RawPortEntry>,
    detailed: bool,
    docker_map_override: Option<HashMap<u16, DockerInfo>>,
) -> Vec<PortInfo> {
    enrich_port_entries_with_detectors(
        scanner,
        entries,
        detailed,
        docker_map_override,
        docker::batch_docker_info,
        find_project_root,
        detect_framework,
    )
}

fn enrich_port_entries_with_detectors<DockerMap, FindRoot, DetectFramework>(
    scanner: &dyn PlatformScanner,
    entries: Vec<RawPortEntry>,
    detailed: bool,
    docker_map_override: Option<HashMap<u16, DockerInfo>>,
    docker_map_provider: DockerMap,
    find_root: FindRoot,
    detect_framework_fn: DetectFramework,
) -> Vec<PortInfo>
where
    DockerMap: Fn() -> HashMap<u16, DockerInfo> + Send,
    FindRoot: Fn(&Path) -> std::path::PathBuf,
    DetectFramework: Fn(&Path) -> Option<String>,
{
    enrich_port_entries_with_detectors_inner(
        scanner,
        entries,
        detailed,
        docker_map_override,
        docker_map_provider,
        find_root,
        detect_framework_fn,
    )
}

fn enrich_port_entries_with_detectors_inner<DockerMap, FindRoot, DetectFramework>(
    scanner: &dyn PlatformScanner,
    entries: Vec<RawPortEntry>,
    detailed: bool,
    docker_map_override: Option<HashMap<u16, DockerInfo>>,
    docker_map_provider: DockerMap,
    find_root: FindRoot,
    detect_framework_fn: DetectFramework,
) -> Vec<PortInfo>
where
    DockerMap: Fn() -> HashMap<u16, DockerInfo> + Send,
    FindRoot: Fn(&Path) -> std::path::PathBuf,
    DetectFramework: Fn(&Path) -> Option<String>,
{
    let pids: Vec<u32> = entries
        .iter()
        .map(|e| e.pid)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let single_process_map = if detailed && entries.len() == 1 {
        entries
            .first()
            .and_then(|entry| {
                scanner
                    .get_process_details(entry.pid)
                    .map(|details| (entry.pid, details))
            })
            .into_iter()
            .collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    let has_docker = entries
        .iter()
        .any(|e| e.process_name.starts_with("com.docke") || e.process_name == "docker");
    let (ps_map, cwd_map, docker_map) = thread::scope(|scope| {
        let ps_task = scope.spawn(|| {
            if single_process_map.is_empty() {
                scanner.batch_process_info(&pids)
            } else {
                single_process_map.clone()
            }
        });
        let cwd_task = scope.spawn(|| scanner.batch_cwd(&pids));
        let docker_task = match docker_map_override {
            Some(_) => None,
            None if has_docker => Some(scope.spawn(docker_map_provider)),
            None => None,
        };
        (
            ps_task
                .join()
                .expect("process collector thread should not panic"),
            cwd_task
                .join()
                .expect("cwd collector thread should not panic"),
            match docker_task {
                Some(task) => task
                    .join()
                    .expect("docker collector thread should not panic"),
                None => Default::default(),
            },
        )
    });
    let docker_map = match docker_map_override {
        Some(map) => map,
        None => docker_map,
    };

    let mut results = Vec::new();
    let mut root_cache: HashMap<std::path::PathBuf, std::path::PathBuf> = HashMap::new();
    let mut framework_cache: HashMap<std::path::PathBuf, Option<String>> = HashMap::new();
    for entry in entries {
        let ps = ps_map.get(&entry.pid);
        let cwd = cwd_map.get(&entry.pid);
        let mut info = PortInfo {
            port: entry.port,
            pid: entry.pid,
            process_name: entry.process_name.clone(),
            raw_name: entry.process_name.clone(),
            command: ps.map(|p| p.command.clone()).unwrap_or_default(),
            cwd: None,
            project_name: None,
            framework: None,
            uptime: None,
            start_time: None,
            status: ProcessStatus::Healthy,
            memory: None,
            git_branch: None,
            process_tree: Vec::new(),
        };

        if let Some(ps) = ps {
            if ps.stat.contains('Z') {
                info.status = ProcessStatus::Zombie;
            } else if ps.ppid == Some(1) && is_dev_process(&entry.process_name, &ps.command) {
                info.status = ProcessStatus::Orphaned;
            }
            if ps.rss_kb > 0 {
                info.memory = Some(format_memory(ps.rss_kb));
            }
            if let Some(lstart) = &ps.lstart {
                info.start_time = lstart.parse::<DisplayTime>().ok();
                info.uptime = format_uptime_from_lstart(lstart);
            }
            info.framework = detect_framework_from_command(&ps.command, &entry.process_name);
        }

        let docker = docker_map.get(&entry.port);
        if let Some(docker) = docker {
            info.project_name = Some(docker.container_name.clone());
            info.framework = Some(docker.framework.clone());
            info.process_name = "docker".to_string();
        }

        if let Some(cwd) = cwd
            && docker.is_none()
        {
            let project_root = root_cache
                .entry(cwd.clone())
                .or_insert_with(|| find_root(cwd))
                .clone();
            info.cwd = Some(project_root.clone());
            info.project_name = path_basename(&project_root);
            if info.framework.is_none() {
                info.framework = framework_cache
                    .entry(project_root.clone())
                    .or_insert_with(|| detect_framework_fn(&project_root))
                    .clone();
            }
            if detailed {
                info.git_branch = git_branch(&project_root);
            }
        }

        if detailed {
            info.process_tree = scanner.get_process_tree(entry.pid);
        }
        results.push(info);
    }
    results.sort_by_key(|p| p.port);
    results
}

fn git_branch(root: &Path) -> Option<String> {
    run_output(
        "git",
        [
            "-C",
            root.to_string_lossy().as_ref(),
            "rev-parse",
            "--abbrev-ref",
            "HEAD",
        ],
        Some(std::time::Duration::from_millis(3000)),
    )
    .ok()
    .filter(|s| !s.is_empty() && s != "HEAD")
}

#[cfg(test)]
mod tests {
    use super::{
        enrich_port_entries_with_detectors, get_listening_ports_with, get_port_details_with,
    };
    use crate::framework::detect_framework;
    use crate::framework::is_dev_process;
    use crate::model::{
        DockerInfo, LogFile, ProcessStatus, ProcessTreeNode, RawPortEntry, RawProcessDetails,
        RawProcessEntry,
    };
    use crate::platform::PlatformScanner;
    use crate::test_support::FakePlatformScanner;
    use crate::util::find_project_root;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn fake_platform_enriches_ports_default_filter_all_and_details() {
        let project = temp_project("ports-fake");
        fs::write(
            project.join("package.json"),
            r#"{"dependencies":{"vite":"latest"}}"#,
        )
        .unwrap();

        let mut fake = FakePlatformScanner {
            listening_ports: vec![
                RawPortEntry {
                    port: 5000,
                    pid: 50,
                    process_name: "Spotify".to_string(),
                },
                RawPortEntry {
                    port: 3000,
                    pid: 42,
                    process_name: "node".to_string(),
                },
                RawPortEntry {
                    port: 5432,
                    pid: 99,
                    process_name: "com.docker.backend".to_string(),
                },
            ],
            ..Default::default()
        };
        fake.process_details.insert(
            42,
            RawProcessDetails {
                pid: 42,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 2048,
                lstart: Some("Jan 01 00:00:00 2000".to_string()),
                command: "node /repo/server.js".to_string(),
            },
        );
        fake.process_details.insert(
            50,
            RawProcessDetails {
                pid: 50,
                ppid: Some(2),
                stat: "S".to_string(),
                rss_kb: 1024,
                lstart: None,
                command: "Spotify".to_string(),
            },
        );
        fake.process_details.insert(
            99,
            RawProcessDetails {
                pid: 99,
                ppid: Some(2),
                stat: "S".to_string(),
                rss_kb: 4096,
                lstart: None,
                command: "com.docker.backend".to_string(),
            },
        );
        fake.cwd.insert(42, project.clone());
        fake.process_trees.insert(
            42,
            vec![ProcessTreeNode {
                pid: 42,
                ppid: Some(1),
                name: "node".to_string(),
            }],
        );

        let docker_map = HashMap::from([(
            5432,
            DockerInfo {
                host_port: 5432,
                container_name: "pg".to_string(),
                image: "postgres:16".to_string(),
                framework: "PostgreSQL".to_string(),
            },
        )]);

        let all_ports = get_listening_ports_with(&fake, false, Some(docker_map.clone()));
        assert_eq!(
            all_ports.iter().map(|p| p.port).collect::<Vec<_>>(),
            vec![3000, 5000, 5432]
        );
        let default_ports: Vec<_> = all_ports
            .iter()
            .filter(|p| is_dev_process(&p.process_name, &p.command))
            .map(|p| p.port)
            .collect();
        assert_eq!(default_ports, vec![3000, 5432]);

        let expected_project = project_name(&project);
        let node = all_ports.iter().find(|p| p.port == 3000).unwrap();
        assert_eq!(
            node.project_name.as_deref(),
            Some(expected_project.as_str())
        );
        assert_eq!(node.framework.as_deref(), Some("Node.js"));
        assert_eq!(node.status, ProcessStatus::Orphaned);
        assert_eq!(node.memory.as_deref(), Some("2.0 MB"));

        let docker = all_ports.iter().find(|p| p.port == 5432).unwrap();
        assert_eq!(docker.process_name, "docker");
        assert_eq!(docker.project_name.as_deref(), Some("pg"));
        assert_eq!(docker.framework.as_deref(), Some("PostgreSQL"));

        let detail = get_port_details_with(&fake, 3000, Some(docker_map)).unwrap();
        assert_eq!(detail.port, 3000);
        assert_eq!(detail.process_tree.len(), 1);

        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn port_field_semantics_match_node_reference_enrichment() {
        let project = temp_project("ports-semantics");
        fs::write(
            project.join("package.json"),
            r#"{"dependencies":{"vite":"latest"}}"#,
        )
        .unwrap();

        let mut fake = FakePlatformScanner {
            listening_ports: vec![
                RawPortEntry {
                    port: 3000,
                    pid: 42,
                    process_name: "node".to_string(),
                },
                RawPortEntry {
                    port: 5432,
                    pid: 99,
                    process_name: "com.docker.backend".to_string(),
                },
            ],
            ..Default::default()
        };
        fake.process_details.insert(
            42,
            RawProcessDetails {
                pid: 42,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 2048,
                lstart: Some("Fri Apr 17 10:00:00 2026".to_string()),
                command: "node /repo/server.js".to_string(),
            },
        );
        fake.process_details.insert(
            99,
            RawProcessDetails {
                pid: 99,
                ppid: Some(2),
                stat: "S".to_string(),
                rss_kb: 4096,
                lstart: None,
                command: "com.docker.backend".to_string(),
            },
        );
        fake.cwd.insert(42, project.clone());

        let docker_map = HashMap::from([(
            5432,
            DockerInfo {
                host_port: 5432,
                container_name: "pg".to_string(),
                image: "postgres:16".to_string(),
                framework: "PostgreSQL".to_string(),
            },
        )]);

        let ports = get_listening_ports_with(&fake, false, Some(docker_map));
        let node = ports.iter().find(|p| p.port == 3000).unwrap();
        assert_eq!(node.process_name, "node");
        assert_eq!(node.raw_name, "node");
        assert_eq!(
            node.project_name.as_deref(),
            Some(project_name(&project).as_str())
        );
        assert_eq!(node.framework.as_deref(), Some("Node.js"));
        assert_eq!(node.status, ProcessStatus::Orphaned);
        assert_eq!(node.memory.as_deref(), Some("2.0 MB"));
        assert_eq!(
            node.start_time.as_ref().map(ToString::to_string).as_deref(),
            Some("Fri Apr 17 10:00:00 2026")
        );
        assert!(node.uptime.is_some());

        let docker = ports.iter().find(|p| p.port == 5432).unwrap();
        assert_eq!(docker.process_name, "docker");
        assert_eq!(docker.raw_name, "com.docker.backend");
        assert_eq!(docker.project_name.as_deref(), Some("pg"));
        assert_eq!(docker.framework.as_deref(), Some("PostgreSQL"));

        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn port_detail_only_enriches_target_port() {
        let mut inner = FakePlatformScanner {
            listening_ports: vec![
                RawPortEntry {
                    port: 3000,
                    pid: 42,
                    process_name: "node".to_string(),
                },
                RawPortEntry {
                    port: 4000,
                    pid: 50,
                    process_name: "node".to_string(),
                },
            ],
            ..Default::default()
        };
        inner.process_details.insert(
            42,
            RawProcessDetails {
                pid: 42,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 2048,
                lstart: Some("Jan 01 00:00:00 2000".to_string()),
                command: "node a.js".to_string(),
            },
        );
        inner.process_details.insert(
            50,
            RawProcessDetails {
                pid: 50,
                ppid: Some(2),
                stat: "S".to_string(),
                rss_kb: 1024,
                lstart: None,
                command: "node b.js".to_string(),
            },
        );
        inner.process_trees.insert(
            42,
            vec![ProcessTreeNode {
                pid: 42,
                ppid: Some(1),
                name: "node".to_string(),
            }],
        );

        let fake = CountingScanner {
            inner,
            batch_process_calls: Mutex::new(Vec::new()),
            batch_cwd_calls: Mutex::new(Vec::new()),
            process_tree_calls: Mutex::new(Vec::new()),
        };

        let detail = get_port_details_with(&fake, 3000, None).expect("detail should exist");

        assert_eq!(detail.port, 3000);
        assert_eq!(
            fake.batch_process_calls.lock().unwrap().as_slice(),
            &[vec![42]]
        );
        assert_eq!(fake.batch_cwd_calls.lock().unwrap().as_slice(), &[vec![42]]);
        assert_eq!(fake.process_tree_calls.lock().unwrap().as_slice(), &[42]);
    }

    #[test]
    fn port_detail_uses_direct_port_lookup_when_scanner_supports_it() {
        let fake = DirectPortLookupScanner {
            inner: FakePlatformScanner {
                listening_ports: vec![RawPortEntry {
                    port: 3000,
                    pid: 42,
                    process_name: "node".to_string(),
                }],
                process_details: HashMap::from([(
                    42,
                    RawProcessDetails {
                        pid: 42,
                        ppid: Some(1),
                        stat: "S".to_string(),
                        rss_kb: 1024,
                        lstart: None,
                        command: "node server.js".to_string(),
                    },
                )]),
                ..Default::default()
            },
            full_scan_calls: Mutex::new(0),
            direct_lookup_calls: Mutex::new(Vec::new()),
        };

        let info = get_port_details_with(&fake, 3000, None).expect("detail should exist");

        assert_eq!(info.port, 3000);
        assert_eq!(*fake.full_scan_calls.lock().unwrap(), 0);
        assert_eq!(fake.direct_lookup_calls.lock().unwrap().as_slice(), &[3000]);
    }

    #[test]
    fn port_detail_prefers_single_process_details_when_scanner_supports_it() {
        let fake = DirectProcessDetailsScanner {
            inner: FakePlatformScanner {
                listening_ports: vec![RawPortEntry {
                    port: 3000,
                    pid: 42,
                    process_name: "node".to_string(),
                }],
                process_details: HashMap::from([(
                    42,
                    RawProcessDetails {
                        pid: 42,
                        ppid: Some(1),
                        stat: "S".to_string(),
                        rss_kb: 1024,
                        lstart: None,
                        command: "node server.js".to_string(),
                    },
                )]),
                ..Default::default()
            },
            batch_process_calls: Mutex::new(Vec::new()),
            single_process_calls: Mutex::new(Vec::new()),
        };

        let info = get_port_details_with(&fake, 3000, None).expect("detail should exist");

        assert_eq!(info.port, 3000);
        assert!(fake.batch_process_calls.lock().unwrap().is_empty());
        assert_eq!(fake.single_process_calls.lock().unwrap().as_slice(), &[42]);
    }

    #[test]
    fn listening_ports_helper_returns_ports_directly() {
        let fake = FakePlatformScanner {
            listening_ports: vec![RawPortEntry {
                port: 3000,
                pid: 42,
                process_name: "node".to_string(),
            }],
            process_details: HashMap::from([(
                42,
                RawProcessDetails {
                    pid: 42,
                    ppid: Some(1),
                    stat: "S".to_string(),
                    rss_kb: 1024,
                    lstart: None,
                    command: "node server.js".to_string(),
                },
            )]),
            ..Default::default()
        };

        let result = get_listening_ports_with(&fake, false, None);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].port, 3000);
    }

    #[test]
    fn port_detail_helper_returns_optional_info_directly() {
        let fake = FakePlatformScanner {
            listening_ports: vec![RawPortEntry {
                port: 3000,
                pid: 42,
                process_name: "node".to_string(),
            }],
            process_details: HashMap::from([(
                42,
                RawProcessDetails {
                    pid: 42,
                    ppid: Some(1),
                    stat: "S".to_string(),
                    rss_kb: 1024,
                    lstart: None,
                    command: "node server.js".to_string(),
                },
            )]),
            ..Default::default()
        };

        let result = get_port_details_with(&fake, 3000, None);

        assert_eq!(result.as_ref().map(|info| info.port), Some(3000));
    }

    #[test]
    fn port_info_start_time_is_stored_as_datetime_equivalent() {
        let mut fake = FakePlatformScanner {
            listening_ports: vec![RawPortEntry {
                port: 3000,
                pid: 42,
                process_name: "node".to_string(),
            }],
            ..Default::default()
        };
        fake.process_details.insert(
            42,
            RawProcessDetails {
                pid: 42,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 2048,
                lstart: Some("Fri Apr 17 10:00:00 2026".to_string()),
                command: "node server.js".to_string(),
            },
        );

        let detail = get_port_details_with(&fake, 3000, None).expect("detail should exist");

        let started = detail.start_time.expect("start time should be present");
        assert_eq!(started.year, 2026);
        assert_eq!(started.month, 4);
        assert_eq!(started.day, 17);
        assert_eq!(started.hour, 10);
        assert_eq!(started.minute, 0);
        assert_eq!(started.second, 0);
        assert_eq!(started.to_string(), "Fri Apr 17 10:00:00 2026");
    }

    #[test]
    fn non_docker_ports_do_not_invoke_docker_provider() {
        let mut fake = FakePlatformScanner {
            listening_ports: vec![RawPortEntry {
                port: 3000,
                pid: 42,
                process_name: "node".to_string(),
            }],
            ..Default::default()
        };
        fake.process_details.insert(
            42,
            RawProcessDetails {
                pid: 42,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 2048,
                lstart: Some("Jan 01 00:00:00 2000".to_string()),
                command: "node server.js".to_string(),
            },
        );

        let docker_calls = AtomicUsize::new(0);
        let ports = enrich_port_entries_with_detectors(
            &fake,
            fake.listening_ports.clone(),
            false,
            None,
            || {
                docker_calls.fetch_add(1, Ordering::SeqCst);
                HashMap::new()
            },
            find_project_root,
            detect_framework,
        );

        assert_eq!(ports.len(), 1);
        assert_eq!(docker_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn repeated_cwd_reuses_project_root_and_framework_detection() {
        let shared_root = temp_project("shared-root-cache");
        fs::write(
            shared_root.join("package.json"),
            r#"{"dependencies":{"vite":"latest"}}"#,
        )
        .unwrap();
        let nested = shared_root.join("apps/web");
        fs::create_dir_all(&nested).unwrap();

        let mut fake = FakePlatformScanner {
            listening_ports: vec![
                RawPortEntry {
                    port: 3000,
                    pid: 42,
                    process_name: "custom-dev".to_string(),
                },
                RawPortEntry {
                    port: 3001,
                    pid: 43,
                    process_name: "custom-dev".to_string(),
                },
            ],
            ..Default::default()
        };
        fake.process_details.insert(
            42,
            RawProcessDetails {
                pid: 42,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 1024,
                lstart: None,
                command: "custom-dev /repo/a.js".to_string(),
            },
        );
        fake.process_details.insert(
            43,
            RawProcessDetails {
                pid: 43,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 1024,
                lstart: None,
                command: "custom-dev /repo/b.js".to_string(),
            },
        );
        fake.cwd.insert(42, nested.clone());
        fake.cwd.insert(43, nested.clone());

        let root_calls = AtomicUsize::new(0);
        let framework_calls = AtomicUsize::new(0);
        let ports = enrich_port_entries_with_detectors(
            &fake,
            fake.listening_ports.clone(),
            false,
            None,
            HashMap::new,
            |cwd| {
                root_calls.fetch_add(1, Ordering::SeqCst);
                find_project_root(cwd)
            },
            |root| {
                framework_calls.fetch_add(1, Ordering::SeqCst);
                detect_framework(root)
            },
        );

        assert_eq!(ports.len(), 2);
        assert_eq!(root_calls.load(Ordering::SeqCst), 1);
        assert_eq!(framework_calls.load(Ordering::SeqCst), 1);
        fs::remove_dir_all(shared_root).unwrap();
    }

    #[test]
    fn listener_enrichment_starts_process_and_cwd_collectors_concurrently() {
        let barrier = Arc::new(Barrier::new(3));
        let process_started = Arc::new(AtomicBool::new(false));
        let cwd_started = Arc::new(AtomicBool::new(false));

        let fake = BlockingScanner {
            inner: FakePlatformScanner {
                listening_ports: vec![RawPortEntry {
                    port: 3000,
                    pid: 42,
                    process_name: "node".to_string(),
                }],
                process_details: HashMap::from([(
                    42,
                    RawProcessDetails {
                        pid: 42,
                        ppid: Some(1),
                        stat: "S".to_string(),
                        rss_kb: 2048,
                        lstart: None,
                        command: "node server.js".to_string(),
                    },
                )]),
                ..Default::default()
            },
            barrier: barrier.clone(),
            process_started: process_started.clone(),
            cwd_started: cwd_started.clone(),
        };

        let handle = std::thread::spawn(move || get_listening_ports_with(&fake, false, None));

        barrier.wait();
        let ports = handle.join().expect("concurrent enrichment should finish");

        assert_eq!(ports.len(), 1);
        assert!(process_started.load(Ordering::SeqCst));
        assert!(cwd_started.load(Ordering::SeqCst));
    }

    #[test]
    fn docker_mapping_joins_parallel_collectors_only_for_docker_like_listener() {
        let barrier = Arc::new(Barrier::new(4));
        let process_started = Arc::new(AtomicBool::new(false));
        let cwd_started = Arc::new(AtomicBool::new(false));
        let docker_started = Arc::new(AtomicBool::new(false));

        let fake = BlockingScanner {
            inner: FakePlatformScanner {
                listening_ports: vec![RawPortEntry {
                    port: 5432,
                    pid: 99,
                    process_name: "com.docker.backend".to_string(),
                }],
                process_details: HashMap::from([(
                    99,
                    RawProcessDetails {
                        pid: 99,
                        ppid: Some(1),
                        stat: "S".to_string(),
                        rss_kb: 4096,
                        lstart: None,
                        command: "com.docker.backend".to_string(),
                    },
                )]),
                ..Default::default()
            },
            barrier: barrier.clone(),
            process_started: process_started.clone(),
            cwd_started: cwd_started.clone(),
        };

        let docker_barrier = barrier.clone();
        let docker_flag = docker_started.clone();
        let handle = std::thread::spawn(move || {
            enrich_port_entries_with_detectors(
                &fake,
                fake.inner.listening_ports.clone(),
                false,
                None,
                || {
                    docker_flag.store(true, Ordering::SeqCst);
                    docker_barrier.wait();
                    HashMap::from([(
                        5432,
                        DockerInfo {
                            host_port: 5432,
                            container_name: "pg".to_string(),
                            image: "postgres:16".to_string(),
                            framework: "PostgreSQL".to_string(),
                        },
                    )])
                },
                find_project_root,
                detect_framework,
            )
        });

        barrier.wait();
        let ports = handle.join().expect("docker enrichment should finish");

        assert_eq!(ports.len(), 1);
        assert!(process_started.load(Ordering::SeqCst));
        assert!(cwd_started.load(Ordering::SeqCst));
        assert!(docker_started.load(Ordering::SeqCst));
        assert_eq!(ports[0].framework.as_deref(), Some("PostgreSQL"));
    }

    struct CountingScanner {
        inner: FakePlatformScanner,
        batch_process_calls: Mutex<Vec<Vec<u32>>>,
        batch_cwd_calls: Mutex<Vec<Vec<u32>>>,
        process_tree_calls: Mutex<Vec<u32>>,
    }

    struct BlockingScanner {
        inner: FakePlatformScanner,
        barrier: Arc<Barrier>,
        process_started: Arc<AtomicBool>,
        cwd_started: Arc<AtomicBool>,
    }

    struct DirectPortLookupScanner {
        inner: FakePlatformScanner,
        full_scan_calls: Mutex<u32>,
        direct_lookup_calls: Mutex<Vec<u16>>,
    }

    struct DirectProcessDetailsScanner {
        inner: FakePlatformScanner,
        batch_process_calls: Mutex<Vec<Vec<u32>>>,
        single_process_calls: Mutex<Vec<u32>>,
    }

    impl PlatformScanner for CountingScanner {
        fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
            self.inner.get_listening_ports_raw()
        }

        fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
            self.batch_process_calls.lock().unwrap().push(pids.to_vec());
            self.inner.batch_process_info(pids)
        }

        fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf> {
            self.batch_cwd_calls.lock().unwrap().push(pids.to_vec());
            self.inner.batch_cwd(pids)
        }

        fn get_all_processes_raw(&self) -> Vec<RawProcessEntry> {
            self.inner.get_all_processes_raw()
        }

        fn get_process_tree(&self, pid: u32) -> Vec<ProcessTreeNode> {
            self.process_tree_calls.lock().unwrap().push(pid);
            self.inner.get_process_tree(pid)
        }

        fn pid_exists(&self, pid: u32) -> bool {
            self.inner.pid_exists(pid)
        }

        fn kill_process(&self, pid: u32, signal: &str) -> bool {
            self.inner.kill_process(pid, signal)
        }

        fn get_process_log_files(&self, pid: u32) -> Vec<LogFile> {
            self.inner.get_process_log_files(pid)
        }

        fn get_system_log_command(&self, pid: u32, follow: bool) -> Option<String> {
            self.inner.get_system_log_command(pid, follow)
        }
    }

    impl PlatformScanner for BlockingScanner {
        fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
            self.inner.get_listening_ports_raw()
        }

        fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
            self.process_started.store(true, Ordering::SeqCst);
            self.barrier.wait();
            self.inner.batch_process_info(pids)
        }

        fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf> {
            self.cwd_started.store(true, Ordering::SeqCst);
            self.barrier.wait();
            self.inner.batch_cwd(pids)
        }

        fn get_all_processes_raw(&self) -> Vec<RawProcessEntry> {
            self.inner.get_all_processes_raw()
        }

        fn get_process_tree(&self, pid: u32) -> Vec<ProcessTreeNode> {
            self.inner.get_process_tree(pid)
        }

        fn pid_exists(&self, pid: u32) -> bool {
            self.inner.pid_exists(pid)
        }

        fn kill_process(&self, pid: u32, signal: &str) -> bool {
            self.inner.kill_process(pid, signal)
        }

        fn get_process_log_files(&self, pid: u32) -> Vec<LogFile> {
            self.inner.get_process_log_files(pid)
        }

        fn get_system_log_command(&self, pid: u32, follow: bool) -> Option<String> {
            self.inner.get_system_log_command(pid, follow)
        }
    }

    impl PlatformScanner for DirectPortLookupScanner {
        fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
            *self.full_scan_calls.lock().unwrap() += 1;
            self.inner.get_listening_ports_raw()
        }

        fn get_listening_port_raw(&self, port: u16) -> Option<RawPortEntry> {
            self.direct_lookup_calls.lock().unwrap().push(port);
            self.inner
                .get_listening_ports_raw()
                .into_iter()
                .find(|entry| entry.port == port)
        }

        fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
            self.inner.batch_process_info(pids)
        }

        fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf> {
            self.inner.batch_cwd(pids)
        }

        fn get_all_processes_raw(&self) -> Vec<RawProcessEntry> {
            self.inner.get_all_processes_raw()
        }

        fn get_process_tree(&self, pid: u32) -> Vec<ProcessTreeNode> {
            self.inner.get_process_tree(pid)
        }

        fn pid_exists(&self, pid: u32) -> bool {
            self.inner.pid_exists(pid)
        }

        fn kill_process(&self, pid: u32, signal: &str) -> bool {
            self.inner.kill_process(pid, signal)
        }

        fn get_process_log_files(&self, pid: u32) -> Vec<LogFile> {
            self.inner.get_process_log_files(pid)
        }

        fn get_system_log_command(&self, pid: u32, follow: bool) -> Option<String> {
            self.inner.get_system_log_command(pid, follow)
        }
    }

    impl PlatformScanner for DirectProcessDetailsScanner {
        fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
            self.inner.get_listening_ports_raw()
        }

        fn get_listening_port_raw(&self, port: u16) -> Option<RawPortEntry> {
            self.inner
                .get_listening_ports_raw()
                .into_iter()
                .find(|entry| entry.port == port)
        }

        fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
            self.batch_process_calls.lock().unwrap().push(pids.to_vec());
            self.inner.batch_process_info(pids)
        }

        fn get_process_details(&self, pid: u32) -> Option<RawProcessDetails> {
            self.single_process_calls.lock().unwrap().push(pid);
            self.inner.process_details.get(&pid).cloned()
        }

        fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf> {
            self.inner.batch_cwd(pids)
        }

        fn get_all_processes_raw(&self) -> Vec<RawProcessEntry> {
            self.inner.get_all_processes_raw()
        }

        fn get_process_tree(&self, pid: u32) -> Vec<ProcessTreeNode> {
            self.inner.get_process_tree(pid)
        }

        fn pid_exists(&self, pid: u32) -> bool {
            self.inner.pid_exists(pid)
        }

        fn kill_process(&self, pid: u32, signal: &str) -> bool {
            self.inner.kill_process(pid, signal)
        }

        fn get_process_log_files(&self, pid: u32) -> Vec<LogFile> {
            self.inner.get_process_log_files(pid)
        }

        fn get_system_log_command(&self, pid: u32, follow: bool) -> Option<String> {
            self.inner.get_system_log_command(pid, follow)
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
