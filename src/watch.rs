use crate::display;
use crate::docker;
use crate::error;
use crate::json_output;
use crate::model::DockerInfo;
use crate::model::{PortInfo, RawPortEntry};
use crate::platform::{self, PlatformScanner};
use crate::ports;
use crate::style;
use std::collections::HashMap;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

fn stop_requested() -> bool {
    STOP_REQUESTED.load(Ordering::SeqCst)
}

fn reset_stop_requested() {
    STOP_REQUESTED.store(false, Ordering::SeqCst);
}

#[cfg(test)]
fn set_stop_requested_for_test(value: bool) {
    STOP_REQUESTED.store(value, Ordering::SeqCst);
}

#[derive(Clone, Debug, Default)]
struct DockerWatchCache {
    ports: Vec<u16>,
    map: HashMap<u16, DockerInfo>,
}

#[derive(Clone, Debug)]
pub(crate) enum WatchEvent {
    New(PortInfo),
    Removed(PortInfo),
}

pub fn run_watch(display_config: display::DisplayConfig) -> i32 {
    install_sigint_handler();
    display::display_watch_header_with_config(&display_config);
    run_watch_loop(
        stop_requested,
        |previous, docker_cache| {
            refresh_watch_state_with(
                platform::native_scanner(),
                previous,
                docker_cache,
                docker::batch_docker_info,
            )
        },
        display_watch_warning,
        display::display_watch_event,
        sleep_interruptibly,
        None,
    );
    println!("{}", stopped_message());
    0
}

pub fn run_watch_json() -> i32 {
    install_sigint_handler();
    run_watch_json_loop(
        stop_requested,
        |previous, docker_cache| {
            refresh_watch_state_with(
                platform::native_scanner(),
                previous,
                docker_cache,
                docker::batch_docker_info,
            )
        },
        |line| println!("{line}"),
        sleep_interruptibly,
        None,
    )
}

fn stopped_message() -> String {
    style::gray("\n\n  Stopped watching.\n")
}

fn run_watch_loop<ShouldStop, Refresh, DisplayWarnings, DisplayEvent, Sleep>(
    should_stop: ShouldStop,
    refresh: Refresh,
    display_warning: DisplayWarnings,
    display_event: DisplayEvent,
    sleep: Sleep,
    max_iterations: Option<usize>,
) -> i32
where
    ShouldStop: Fn() -> bool,
    Refresh: Fn(
        &HashMap<u16, PortInfo>,
        &DockerWatchCache,
    ) -> (HashMap<u16, PortInfo>, Vec<WatchEvent>, DockerWatchCache),
    DisplayWarnings: Fn(&error::PortError),
    DisplayEvent: Fn(&str, &PortInfo),
    Sleep: Fn(Duration),
{
    let mut previous: HashMap<u16, _> = HashMap::new();
    let mut docker_cache = DockerWatchCache::default();
    let mut iterations = 0usize;

    error::drain_user_warnings();

    while !should_stop() {
        let (current, events, next_cache) = refresh(&previous, &docker_cache);
        if should_stop() {
            error::drain_user_warnings();
            break;
        }
        let warnings = error::drain_user_warnings();
        for warning in &warnings {
            if should_stop() {
                break;
            }
            display_warning(warning);
        }
        for event in &events {
            if should_stop() {
                break;
            }
            match event {
                WatchEvent::New(info) => display_event("new", info),
                WatchEvent::Removed(info) => display_event("removed", info),
            }
        }
        previous = current;
        docker_cache = next_cache;
        iterations += 1;
        if max_iterations.is_some_and(|max| iterations >= max) {
            break;
        }
        sleep(Duration::from_millis(2000));
    }

    0
}

fn run_watch_json_loop<ShouldStop, Refresh, EmitLine, Sleep>(
    should_stop: ShouldStop,
    refresh: Refresh,
    emit_line: EmitLine,
    sleep: Sleep,
    max_iterations: Option<usize>,
) -> i32
where
    ShouldStop: Fn() -> bool,
    Refresh: Fn(
        &HashMap<u16, PortInfo>,
        &DockerWatchCache,
    ) -> (HashMap<u16, PortInfo>, Vec<WatchEvent>, DockerWatchCache),
    EmitLine: Fn(String),
    Sleep: Fn(Duration),
{
    let mut previous: HashMap<u16, _> = HashMap::new();
    let mut docker_cache = DockerWatchCache::default();
    let mut iterations = 0usize;

    error::drain_user_warnings();

    while !should_stop() {
        let (current, events, next_cache) = refresh(&previous, &docker_cache);
        if should_stop() {
            error::drain_user_warnings();
            break;
        }
        let warnings = error::drain_user_warnings();
        for warning in warnings {
            if should_stop() {
                break;
            }
            if let Ok(line) = json_output::render_json(&json_output::CommandEnvelope::ok(
                "ports watch",
                json_output::watch_warning_payload(warning.user_message()),
            )) {
                emit_line(line);
            }
        }
        for event in events {
            if should_stop() {
                break;
            }
            let (action, info) = match event {
                WatchEvent::New(info) => ("new", info),
                WatchEvent::Removed(info) => ("removed", info),
            };
            if let Ok(line) = json_output::render_json(&json_output::CommandEnvelope::ok(
                "ports watch",
                json_output::watch_event_payload(action, &info),
            )) {
                emit_line(line);
            }
        }
        previous = current;
        docker_cache = next_cache;
        iterations += 1;
        if max_iterations.is_some_and(|max| iterations >= max) {
            break;
        }
        sleep(Duration::from_millis(2000));
    }

    0
}

fn display_watch_warning(warning: &error::PortError) {
    println!("  warning: {}", warning.user_message());
}

fn refresh_watch_state_with<DockerMap>(
    scanner: &dyn PlatformScanner,
    previous: &HashMap<u16, PortInfo>,
    docker_cache: &DockerWatchCache,
    docker_map: DockerMap,
) -> (HashMap<u16, PortInfo>, Vec<WatchEvent>, DockerWatchCache)
where
    DockerMap: Fn() -> HashMap<u16, DockerInfo>,
{
    let entries = scanner.get_listening_ports_raw();
    let next_docker_cache = next_docker_cache(&entries, docker_cache, docker_map);
    let current = build_current_watch_state(scanner, previous, entries, &next_docker_cache.map);
    let events = diff_watch_events(previous, &current);
    (current, events, next_docker_cache)
}

fn build_current_watch_state(
    scanner: &dyn PlatformScanner,
    previous: &HashMap<u16, PortInfo>,
    entries: Vec<RawPortEntry>,
    docker_map: &HashMap<u16, DockerInfo>,
) -> HashMap<u16, PortInfo> {
    let mut current = HashMap::new();
    let mut pending = Vec::new();

    for entry in entries {
        match previous.get(&entry.port) {
            Some(existing) if existing.pid == entry.pid => {
                current.insert(entry.port, existing.clone());
            }
            _ => pending.push(entry),
        }
    }

    let enriched = ports::get_listening_ports_from_entries(
        scanner,
        pending.clone(),
        false,
        Some(docker_map.clone()),
    );
    let mut enriched_by_port: HashMap<u16, PortInfo> =
        enriched.into_iter().map(|info| (info.port, info)).collect();

    for entry in pending {
        if let Some(info) = enriched_by_port.remove(&entry.port) {
            current.insert(entry.port, info);
        }
    }

    current
}

fn next_docker_cache<DockerMap>(
    entries: &[RawPortEntry],
    previous: &DockerWatchCache,
    docker_map: DockerMap,
) -> DockerWatchCache
where
    DockerMap: Fn() -> HashMap<u16, DockerInfo>,
{
    let mut ports: Vec<u16> = entries
        .iter()
        .filter(|entry| {
            entry.process_name.starts_with("com.docke") || entry.process_name == "docker"
        })
        .map(|entry| entry.port)
        .collect();
    ports.sort_unstable();
    ports.dedup();

    if ports == previous.ports {
        return previous.clone();
    }

    DockerWatchCache {
        ports,
        map: docker_map(),
    }
}

pub(crate) fn diff_watch_events(
    previous: &HashMap<u16, PortInfo>,
    current: &HashMap<u16, PortInfo>,
) -> Vec<WatchEvent> {
    let mut events = Vec::new();
    let mut current_ports: Vec<_> = current.keys().copied().collect();
    current_ports.sort_unstable();
    for port in current_ports {
        if !previous.contains_key(&port)
            && let Some(info) = current.get(&port)
        {
            events.push(WatchEvent::New(info.clone()));
        }
    }
    let mut previous_ports: Vec<_> = previous.keys().copied().collect();
    previous_ports.sort_unstable();
    for port in previous_ports {
        if !current.contains_key(&port)
            && let Some(info) = previous.get(&port)
        {
            events.push(WatchEvent::Removed(info.clone()));
        }
    }
    events
}

fn sleep_interruptibly(duration: Duration) {
    let step = Duration::from_millis(100);
    let mut elapsed = Duration::ZERO;
    while elapsed < duration && !stop_requested() {
        thread::sleep(step.min(duration - elapsed));
        elapsed += step;
    }
}

fn install_sigint_handler() {
    reset_stop_requested();
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let _ = ctrlc::set_handler(|| {
            STOP_REQUESTED.store(true, Ordering::SeqCst);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::{
        DockerWatchCache, WatchEvent, diff_watch_events, install_sigint_handler,
        refresh_watch_state_with, reset_stop_requested, run_watch_json_loop, run_watch_loop,
        set_stop_requested_for_test, stop_requested,
    };
    use crate::error::{PortError, drain_user_warnings, record_user_warning, verbose_test_lock};
    use crate::model::{
        DockerInfo, LogFile, PortInfo, ProcessStatus, ProcessTreeNode, RawPortEntry,
        RawProcessDetails, RawProcessEntry,
    };
    use crate::platform::PlatformScanner;
    use crate::style;
    use crate::test_support::FakePlatformScanner;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[test]
    fn watch_diff_detects_new_and_removed_without_sleeping() {
        let previous = HashMap::from([(3000, port(3000)), (4000, port(4000))]);
        let current = HashMap::from([(3000, port(3000)), (5000, port(5000))]);

        let events = diff_watch_events(&previous, &current);
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], WatchEvent::New(info) if info.port == 5000));
        assert!(matches!(&events[1], WatchEvent::Removed(info) if info.port == 4000));
    }

    #[test]
    fn watch_scan_does_not_need_process_enrichment_for_unchanged_ports() {
        let mut inner = FakePlatformScanner {
            listening_ports: vec![
                RawPortEntry {
                    port: 3000,
                    pid: 30,
                    process_name: "node".to_string(),
                },
                RawPortEntry {
                    port: 4000,
                    pid: 40,
                    process_name: "node".to_string(),
                },
            ],
            ..Default::default()
        };
        inner.process_details.insert(
            30,
            RawProcessDetails {
                pid: 30,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 1024,
                lstart: Some("Jan 01 00:00:00 2000".to_string()),
                command: "node old.js".to_string(),
            },
        );
        inner.process_details.insert(
            40,
            RawProcessDetails {
                pid: 40,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 2048,
                lstart: Some("Jan 01 00:00:00 2000".to_string()),
                command: "node new.js".to_string(),
            },
        );

        let fake = CountingScanner {
            inner,
            batch_calls: Mutex::new(Vec::new()),
            cwd_calls: Mutex::new(Vec::new()),
        };

        let previous = HashMap::from([(3000, port_with_pid(3000, 30))]);
        let (current, events, _) =
            refresh_watch_state_with(&fake, &previous, &DockerWatchCache::default(), || {
                HashMap::new()
            });

        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], WatchEvent::New(info) if info.port == 4000));
        assert_eq!(fake.batch_calls.lock().unwrap().as_slice(), &[vec![40]]);
        assert_eq!(fake.cwd_calls.lock().unwrap().as_slice(), &[vec![40]]);
        assert_eq!(
            current.get(&3000).expect("unchanged port kept").command,
            "node server.js"
        );
    }

    #[test]
    fn watch_scan_reenriches_port_when_pid_changes() {
        let mut inner = FakePlatformScanner {
            listening_ports: vec![RawPortEntry {
                port: 3000,
                pid: 31,
                process_name: "node".to_string(),
            }],
            ..Default::default()
        };
        inner.process_details.insert(
            31,
            RawProcessDetails {
                pid: 31,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 4096,
                lstart: Some("Jan 01 00:00:00 2000".to_string()),
                command: "node changed.js".to_string(),
            },
        );

        let fake = CountingScanner {
            inner,
            batch_calls: Mutex::new(Vec::new()),
            cwd_calls: Mutex::new(Vec::new()),
        };

        let previous = HashMap::from([(3000, port_with_pid(3000, 30))]);
        let (current, events, _) =
            refresh_watch_state_with(&fake, &previous, &DockerWatchCache::default(), || {
                HashMap::new()
            });

        assert!(events.is_empty());
        assert_eq!(fake.batch_calls.lock().unwrap().as_slice(), &[vec![31]]);
        assert_eq!(current.get(&3000).expect("changed pid refreshed").pid, 31);
        assert_eq!(
            current.get(&3000).expect("changed pid refreshed").command,
            "node changed.js"
        );
    }

    #[test]
    fn watch_diff_matches_node_reference_by_comparing_port_sets_only() {
        let previous = HashMap::from([(3000, port_with_pid(3000, 30))]);
        let current = HashMap::from([(3000, port_with_pid(3000, 31))]);

        let events = diff_watch_events(&previous, &current);

        assert!(events.is_empty());
    }

    #[test]
    fn watch_scan_reuses_cached_docker_mapping_when_docker_ports_unchanged() {
        let mut inner = FakePlatformScanner {
            listening_ports: vec![RawPortEntry {
                port: 5432,
                pid: 100,
                process_name: "com.docker.backend".to_string(),
            }],
            ..Default::default()
        };
        inner.process_details.insert(
            100,
            RawProcessDetails {
                pid: 100,
                ppid: Some(1),
                stat: "S".to_string(),
                rss_kb: 4096,
                lstart: Some("Jan 01 00:00:00 2000".to_string()),
                command: "com.docker.backend".to_string(),
            },
        );

        let fake = CountingScanner {
            inner,
            batch_calls: Mutex::new(Vec::new()),
            cwd_calls: Mutex::new(Vec::new()),
        };
        let previous = HashMap::from([(5432, docker_port_with_pid(5432, 99))]);
        let cache = DockerWatchCache {
            ports: vec![5432],
            map: HashMap::from([(
                5432,
                DockerInfo {
                    host_port: 5432,
                    container_name: "pg".to_string(),
                    image: "postgres:16".to_string(),
                    framework: "PostgreSQL".to_string(),
                },
            )]),
        };
        let docker_calls = RefCell::new(0usize);

        let (current, events, next_cache) =
            refresh_watch_state_with(&fake, &previous, &cache, || {
                *docker_calls.borrow_mut() += 1;
                HashMap::new()
            });

        assert!(events.is_empty());
        assert_eq!(*docker_calls.borrow(), 0);
        assert_eq!(next_cache.ports, vec![5432]);
        let info = current.get(&5432).expect("docker port refreshed");
        assert_eq!(info.project_name.as_deref(), Some("pg"));
        assert_eq!(info.framework.as_deref(), Some("PostgreSQL"));
    }

    #[test]
    fn watch_loop_stops_after_interrupting_sleep() {
        let stopped = AtomicBool::new(false);
        let refreshes = AtomicUsize::new(0);
        let sleeps = AtomicUsize::new(0);

        let exit = run_watch_loop(
            || stopped.load(Ordering::SeqCst),
            |_, _| {
                refreshes.fetch_add(1, Ordering::SeqCst);
                (HashMap::new(), Vec::new(), DockerWatchCache::default())
            },
            |_| {},
            |_, _| {},
            |_| {
                sleeps.fetch_add(1, Ordering::SeqCst);
                stopped.store(true, Ordering::SeqCst);
            },
            None,
        );

        assert_eq!(exit, 0);
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);
        assert_eq!(sleeps.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn watch_loop_does_not_refresh_again_after_interruptible_sleep_sets_stop_flag() {
        let _guard = verbose_test_lock().lock().unwrap();
        reset_stop_requested();
        let refreshes = AtomicUsize::new(0);

        let exit = run_watch_loop(
            stop_requested,
            |_, _| {
                refreshes.fetch_add(1, Ordering::SeqCst);
                (HashMap::new(), Vec::new(), DockerWatchCache::default())
            },
            |_| {},
            |_, _| {},
            |_| {
                set_stop_requested_for_test(true);
            },
            Some(2),
        );

        assert_eq!(exit, 0);
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);
        reset_stop_requested();
    }

    #[test]
    fn sleep_interruptibly_returns_early_when_stop_is_requested() {
        let _guard = verbose_test_lock().lock().unwrap();
        reset_stop_requested();
        let started = Arc::new(AtomicBool::new(false));
        let started_for_thread = Arc::clone(&started);
        let interrupter = std::thread::spawn(move || {
            started_for_thread.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(10));
            set_stop_requested_for_test(true);
        });

        while !started.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }

        let start = Instant::now();
        super::sleep_interruptibly(Duration::from_millis(500));
        let elapsed = start.elapsed();

        interrupter
            .join()
            .expect("interrupter thread should finish");
        assert!(elapsed < Duration::from_millis(500));
        reset_stop_requested();
    }

    #[test]
    fn watch_stop_flag_helpers_clear_previous_interrupt_state() {
        let _guard = verbose_test_lock().lock().unwrap();
        set_stop_requested_for_test(true);
        assert!(stop_requested());

        reset_stop_requested();

        assert!(!stop_requested());
    }

    #[test]
    fn install_sigint_handler_resets_stop_flag_before_new_session_starts() {
        let _guard = verbose_test_lock().lock().unwrap();
        set_stop_requested_for_test(true);
        assert!(stop_requested());

        install_sigint_handler();

        assert!(!stop_requested());
    }

    #[test]
    fn watch_loop_clears_stale_user_warnings_before_refreshing() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        record_user_warning(&PortError::CommandMissing("lsof".into()));

        let exit = run_watch_loop(
            || false,
            |_, _| {
                assert!(drain_user_warnings().is_empty());
                (HashMap::new(), Vec::new(), DockerWatchCache::default())
            },
            |_| {},
            |_, _| {},
            |_| {},
            Some(1),
        );

        assert_eq!(exit, 0);
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn watch_loop_displays_and_drains_user_warnings_after_each_refresh() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        let refreshes = AtomicUsize::new(0);
        let displayed = RefCell::new(Vec::<Vec<String>>::new());

        let exit = run_watch_loop(
            || false,
            |_, _| {
                let next = refreshes.fetch_add(1, Ordering::SeqCst) + 1;
                record_user_warning(&PortError::PermissionDenied(format!("ps-{next}")));
                (HashMap::new(), Vec::new(), DockerWatchCache::default())
            },
            |warning| {
                displayed.borrow_mut().push(vec![warning.user_message()]);
            },
            |_, _| {},
            |_| {
                assert!(drain_user_warnings().is_empty());
            },
            Some(2),
        );

        assert_eq!(exit, 0);
        assert_eq!(refreshes.load(Ordering::SeqCst), 2);
        assert_eq!(
            displayed.into_inner(),
            vec![
                vec!["permission denied while inspecting processes".to_string()],
                vec!["permission denied while inspecting processes".to_string()],
            ]
        );
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn watch_json_outputs_json_lines_for_events_and_warnings() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        let lines = RefCell::new(Vec::<String>::new());
        let refreshes = AtomicUsize::new(0);

        let exit = run_watch_json_loop(
            || false,
            |_, _| {
                let iteration = refreshes.fetch_add(1, Ordering::SeqCst);
                if iteration == 0 {
                    record_user_warning(&PortError::PermissionDenied("ps".into()));
                    (
                        HashMap::from([(3000, port_with_pid(3000, 30))]),
                        vec![WatchEvent::New(port_with_pid(3000, 30))],
                        DockerWatchCache::default(),
                    )
                } else {
                    (
                        HashMap::new(),
                        vec![WatchEvent::Removed(port_with_pid(3000, 30))],
                        DockerWatchCache::default(),
                    )
                }
            },
            |line| lines.borrow_mut().push(line),
            |_| {},
            Some(2),
        );

        assert_eq!(exit, 0);
        let rendered = lines.into_inner();
        assert_eq!(rendered.len(), 3);

        let warning =
            serde_json::from_str::<serde_json::Value>(&rendered[0]).expect("json should parse");
        let new_event =
            serde_json::from_str::<serde_json::Value>(&rendered[1]).expect("json should parse");
        let removed_event =
            serde_json::from_str::<serde_json::Value>(&rendered[2]).expect("json should parse");

        assert_eq!(warning["command"], "ports watch");
        assert_eq!(warning["ok"], true);
        assert_eq!(warning["data"]["type"], "warning");
        assert_eq!(
            warning["data"]["message"],
            "permission denied while inspecting processes"
        );
        assert_eq!(new_event["data"]["type"], "event");
        assert_eq!(new_event["data"]["action"], "new");
        assert_eq!(new_event["data"]["port"]["port"], 3000);
        assert_eq!(removed_event["data"]["action"], "removed");
        assert_eq!(removed_event["data"]["port"]["pid"], 30);
        assert!(drain_user_warnings().is_empty());
    }

    #[test]
    fn watch_loop_skips_events_after_stop_is_requested_during_refresh() {
        let _guard = verbose_test_lock().lock().unwrap();
        reset_stop_requested();
        let displayed = RefCell::new(Vec::<(String, u16)>::new());

        let exit = run_watch_loop(
            stop_requested,
            |_, _| {
                set_stop_requested_for_test(true);
                (
                    HashMap::from([(3000, port_with_pid(3000, 30))]),
                    vec![WatchEvent::New(port_with_pid(3000, 30))],
                    DockerWatchCache::default(),
                )
            },
            |_| {},
            |kind, info| displayed.borrow_mut().push((kind.to_string(), info.port)),
            |_| {},
            Some(1),
        );

        assert_eq!(exit, 0);
        assert!(displayed.into_inner().is_empty());
        reset_stop_requested();
    }

    #[test]
    fn watch_loop_stops_emitting_mid_batch_once_stop_is_requested() {
        let _guard = verbose_test_lock().lock().unwrap();
        reset_stop_requested();
        let warnings = RefCell::new(Vec::<String>::new());
        let events = RefCell::new(Vec::<(String, u16)>::new());

        let exit = run_watch_loop(
            stop_requested,
            |_, _| {
                record_user_warning(&PortError::PermissionDenied("ps-1".into()));
                record_user_warning(&PortError::PermissionDenied("ps-2".into()));
                (
                    HashMap::from([
                        (3000, port_with_pid(3000, 30)),
                        (4000, port_with_pid(4000, 40)),
                    ]),
                    vec![
                        WatchEvent::New(port_with_pid(3000, 30)),
                        WatchEvent::New(port_with_pid(4000, 40)),
                    ],
                    DockerWatchCache::default(),
                )
            },
            |warning| {
                warnings.borrow_mut().push(warning.user_message());
                set_stop_requested_for_test(true);
            },
            |kind, info| events.borrow_mut().push((kind.to_string(), info.port)),
            |_| {},
            Some(1),
        );

        assert_eq!(exit, 0);
        assert_eq!(warnings.into_inner().len(), 1);
        assert!(events.into_inner().is_empty());
        assert!(drain_user_warnings().is_empty());
        reset_stop_requested();
    }

    #[test]
    fn watch_loop_skips_warnings_after_stop_is_requested_during_refresh() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        reset_stop_requested();
        let displayed = RefCell::new(Vec::<Vec<String>>::new());

        let exit = run_watch_loop(
            stop_requested,
            |_, _| {
                record_user_warning(&PortError::PermissionDenied("ps".into()));
                set_stop_requested_for_test(true);
                (HashMap::new(), Vec::new(), DockerWatchCache::default())
            },
            |warning| {
                displayed.borrow_mut().push(vec![warning.user_message()]);
            },
            |_, _| {},
            |_| {},
            Some(1),
        );

        assert_eq!(exit, 0);
        assert!(displayed.into_inner().is_empty());
        assert!(drain_user_warnings().is_empty());
        reset_stop_requested();
    }

    #[test]
    fn watch_json_skips_output_after_stop_is_requested_during_refresh() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        reset_stop_requested();
        let lines = RefCell::new(Vec::<String>::new());

        let exit = run_watch_json_loop(
            stop_requested,
            |_, _| {
                record_user_warning(&PortError::PermissionDenied("ps".into()));
                set_stop_requested_for_test(true);
                (
                    HashMap::from([(3000, port_with_pid(3000, 30))]),
                    vec![WatchEvent::New(port_with_pid(3000, 30))],
                    DockerWatchCache::default(),
                )
            },
            |line| lines.borrow_mut().push(line),
            |_| {},
            Some(1),
        );

        assert_eq!(exit, 0);
        assert!(lines.into_inner().is_empty());
        assert!(drain_user_warnings().is_empty());
        reset_stop_requested();
    }

    #[test]
    fn watch_json_stops_emitting_mid_batch_once_stop_is_requested() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();
        reset_stop_requested();
        let lines = RefCell::new(Vec::<String>::new());

        let exit = run_watch_json_loop(
            stop_requested,
            |_, _| {
                record_user_warning(&PortError::PermissionDenied("ps-1".into()));
                record_user_warning(&PortError::PermissionDenied("ps-2".into()));
                (
                    HashMap::from([
                        (3000, port_with_pid(3000, 30)),
                        (4000, port_with_pid(4000, 40)),
                    ]),
                    vec![
                        WatchEvent::New(port_with_pid(3000, 30)),
                        WatchEvent::New(port_with_pid(4000, 40)),
                    ],
                    DockerWatchCache::default(),
                )
            },
            |line| {
                lines.borrow_mut().push(line);
                set_stop_requested_for_test(true);
            },
            |_| {},
            Some(1),
        );

        assert_eq!(exit, 0);
        assert_eq!(lines.into_inner().len(), 1);
        assert!(drain_user_warnings().is_empty());
        reset_stop_requested();
    }

    #[test]
    fn stopped_message_has_stable_text() {
        assert_eq!(
            super::stopped_message(),
            style::gray("\n\n  Stopped watching.\n")
        );
    }

    struct CountingScanner {
        inner: FakePlatformScanner,
        batch_calls: Mutex<Vec<Vec<u32>>>,
        cwd_calls: Mutex<Vec<Vec<u32>>>,
    }

    impl PlatformScanner for CountingScanner {
        fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
            self.inner.get_listening_ports_raw()
        }

        fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
            self.batch_calls.lock().unwrap().push(pids.to_vec());
            self.inner.batch_process_info(pids)
        }

        fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf> {
            self.cwd_calls.lock().unwrap().push(pids.to_vec());
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

    fn port(port: u16) -> PortInfo {
        port_with_pid(port, port as u32)
    }

    fn docker_port_with_pid(port: u16, pid: u32) -> PortInfo {
        PortInfo {
            port,
            pid,
            process_name: "docker".to_string(),
            raw_name: "com.docker.backend".to_string(),
            command: "com.docker.backend".to_string(),
            cwd: None,
            project_name: Some("pg".to_string()),
            framework: Some("PostgreSQL".to_string()),
            uptime: Some("1m 0s".to_string()),
            start_time: None,
            status: ProcessStatus::Healthy,
            memory: Some("4.0 MB".to_string()),
            git_branch: None,
            process_tree: Vec::new(),
        }
    }

    fn port_with_pid(port: u16, pid: u32) -> PortInfo {
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
}
