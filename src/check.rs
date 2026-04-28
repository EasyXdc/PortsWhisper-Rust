use crate::json_output;
use crate::platform::{self, PlatformScanner};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PortCheckResult {
    pub port: u16,
    pub available: bool,
    pub occupied: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CheckPayload {
    pub ports: Vec<PortCheckResult>,
}

pub fn run_check(ports: &[u16]) -> i32 {
    let results = check_ports(ports);
    print_check_results(&results);
    exit_code(&results)
}

pub fn run_check_json(ports: &[u16]) -> i32 {
    let results = check_ports(ports);
    let exit = exit_code(&results);
    let json_exit = json_output::print_json_output(json_output::render_json(
        &json_output::CommandEnvelope::ok("ports check", check_payload(&results)),
    ));
    if json_exit != 0 { json_exit } else { exit }
}

pub fn check_payload(results: &[PortCheckResult]) -> CheckPayload {
    CheckPayload {
        ports: results.to_vec(),
    }
}

pub fn check_ports(ports: &[u16]) -> Vec<PortCheckResult> {
    check_ports_with(platform::native_scanner(), ports)
}

fn check_ports_with(scanner: &dyn PlatformScanner, ports: &[u16]) -> Vec<PortCheckResult> {
    let mut unique_ports = ports.to_vec();
    unique_ports.sort_unstable();
    unique_ports.dedup();

    let occupied: HashSet<u16> = if unique_ports.len() <= TARGETED_CHECK_LIMIT {
        unique_ports
            .iter()
            .filter_map(|port| scanner.get_listening_port_raw(*port))
            .map(|entry| entry.port)
            .collect()
    } else {
        scanner
            .get_listening_ports_raw()
            .into_iter()
            .map(|entry| entry.port)
            .collect()
    };

    ports
        .iter()
        .copied()
        .map(|port| {
            let port_occupied = occupied.contains(&port);
            PortCheckResult {
                port,
                available: !port_occupied,
                occupied: port_occupied,
            }
        })
        .collect()
}

const TARGETED_CHECK_LIMIT: usize = 8;

fn print_check_results(results: &[PortCheckResult]) {
    for result in results {
        let state = if result.occupied {
            "occupied"
        } else {
            "available"
        };
        println!("{} {state}", result.port);
    }
}

fn exit_code(results: &[PortCheckResult]) -> i32 {
    if results.iter().any(|result| result.occupied) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PortCheckResult, TARGETED_CHECK_LIMIT, check_payload, check_ports, check_ports_with,
    };
    use crate::model::RawPortEntry;
    use crate::platform::PlatformScanner;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CheckScanner {
        listening_ports: Vec<RawPortEntry>,
        full_scans: AtomicUsize,
        targeted_scans: AtomicUsize,
    }

    impl CheckScanner {
        fn new(listening_ports: Vec<RawPortEntry>) -> Self {
            Self {
                listening_ports,
                full_scans: AtomicUsize::new(0),
                targeted_scans: AtomicUsize::new(0),
            }
        }
    }

    impl PlatformScanner for CheckScanner {
        fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
            self.full_scans.fetch_add(1, Ordering::SeqCst);
            self.listening_ports.clone()
        }

        fn get_listening_port_raw(&self, port: u16) -> Option<RawPortEntry> {
            self.targeted_scans.fetch_add(1, Ordering::SeqCst);
            self.listening_ports
                .iter()
                .find(|entry| entry.port == port)
                .cloned()
        }

        fn batch_process_info(
            &self,
            _pids: &[u32],
        ) -> HashMap<u32, crate::model::RawProcessDetails> {
            HashMap::new()
        }

        fn batch_cwd(&self, _pids: &[u32]) -> HashMap<u32, PathBuf> {
            HashMap::new()
        }

        fn get_all_processes_raw(&self) -> Vec<crate::model::RawProcessEntry> {
            Vec::new()
        }

        fn get_process_tree(&self, _pid: u32) -> Vec<crate::model::ProcessTreeNode> {
            Vec::new()
        }

        fn pid_exists(&self, _pid: u32) -> bool {
            false
        }

        fn kill_process(&self, _pid: u32, _signal: &str) -> bool {
            false
        }

        fn get_process_log_files(&self, _pid: u32) -> Vec<crate::model::LogFile> {
            Vec::new()
        }

        fn get_system_log_command(&self, _pid: u32, _follow: bool) -> Option<String> {
            None
        }
    }

    #[test]
    fn marks_ports_as_available_or_occupied() {
        let scanner = CheckScanner::new(vec![RawPortEntry {
            port: 3000,
            pid: 42,
            process_name: "node".to_string(),
        }]);

        assert_eq!(
            check_ports_with(&scanner, &[3000, 5173]),
            vec![
                PortCheckResult {
                    port: 3000,
                    available: false,
                    occupied: true,
                },
                PortCheckResult {
                    port: 5173,
                    available: true,
                    occupied: false,
                },
            ]
        );
    }

    #[test]
    fn small_check_uses_targeted_port_lookups() {
        let scanner = CheckScanner::new(vec![RawPortEntry {
            port: 3000,
            pid: 42,
            process_name: "node".to_string(),
        }]);

        let results = check_ports_with(&scanner, &[3000, 3000, 5173]);

        assert!(results[0].occupied);
        assert!(results[1].occupied);
        assert!(!results[2].occupied);
        assert_eq!(scanner.full_scans.load(Ordering::SeqCst), 0);
        assert_eq!(scanner.targeted_scans.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn large_check_uses_single_full_scan() {
        let scanner = CheckScanner::new(vec![RawPortEntry {
            port: 3000,
            pid: 42,
            process_name: "node".to_string(),
        }]);
        let ports = (1..=(TARGETED_CHECK_LIMIT as u16 + 1)).collect::<Vec<_>>();

        let _ = check_ports_with(&scanner, &ports);

        assert_eq!(scanner.full_scans.load(Ordering::SeqCst), 1);
        assert_eq!(scanner.targeted_scans.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn serializes_check_payload_shape() {
        let payload = check_payload(&[
            PortCheckResult {
                port: 3000,
                available: false,
                occupied: true,
            },
            PortCheckResult {
                port: 5173,
                available: true,
                occupied: false,
            },
        ]);

        assert_eq!(payload.ports.len(), 2);
        assert_eq!(payload.ports[0].port, 3000);
        assert!(!payload.ports[0].available);
        assert!(payload.ports[0].occupied);
        assert_eq!(payload.ports[1].port, 5173);
        assert!(payload.ports[1].available);
        assert!(!payload.ports[1].occupied);
    }

    #[test]
    fn check_ports_handles_empty_input() {
        assert!(check_ports(&[]).is_empty());
    }
}
