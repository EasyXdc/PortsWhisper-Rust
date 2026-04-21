use crate::json_output;
use crate::platform;
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
    match json_output::render_json(&json_output::CommandEnvelope::ok(
        "ports check",
        check_payload(&results),
    )) {
        Ok(output) => {
            println!("{output}");
            exit_code(&results)
        }
        Err(err) => {
            eprintln!("failed to render json for ports check: {err}");
            1
        }
    }
}

pub fn check_payload(results: &[PortCheckResult]) -> CheckPayload {
    CheckPayload {
        ports: results.to_vec(),
    }
}

pub fn check_ports(ports: &[u16]) -> Vec<PortCheckResult> {
    let occupied: HashSet<u16> = platform::listening_ports_raw()
        .into_iter()
        .map(|entry| entry.port)
        .collect();

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
    use super::{PortCheckResult, check_payload, check_ports};
    use crate::model::RawPortEntry;
    use crate::platform::PlatformScanner;
    use std::collections::HashMap;
    use std::path::PathBuf;

    struct CheckScanner {
        listening_ports: Vec<RawPortEntry>,
    }

    impl PlatformScanner for CheckScanner {
        fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
            self.listening_ports.clone()
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

    fn check_ports_with(scanner: &dyn PlatformScanner, ports: &[u16]) -> Vec<PortCheckResult> {
        let occupied: std::collections::HashSet<u16> = scanner
            .get_listening_ports_raw()
            .into_iter()
            .map(|entry| entry.port)
            .collect();

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

    #[test]
    fn marks_ports_as_available_or_occupied() {
        let scanner = CheckScanner {
            listening_ports: vec![RawPortEntry {
                port: 3000,
                pid: 42,
                process_name: "node".to_string(),
            }],
        };

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
