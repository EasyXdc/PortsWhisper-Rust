use crate::model::{KillTargetResolution, PortInfo, ProcessInfo};

pub use crate::framework::{
    detect_framework, detect_framework_from_command, detect_framework_from_image,
    detect_framework_from_name, is_dev_process, is_docker_process, summarize_command,
};

/// Re-export of [`ports::get_listening_ports`].
pub fn get_listening_ports(detailed: bool) -> Vec<PortInfo> {
    crate::ports::get_listening_ports(detailed)
}

/// Re-export of [`ports::get_port_details`].
pub fn get_port_details(port: u16) -> Option<PortInfo> {
    crate::ports::get_port_details(port)
}

/// Return enriched metadata for all running processes.
pub fn get_all_processes() -> Vec<ProcessInfo> {
    crate::process::get_all_processes()
}

/// Return enriched metadata for dev-related processes only.
pub fn get_all_dev_processes() -> Vec<ProcessInfo> {
    crate::process::get_all_dev_processes()
}

/// Return port entries whose processes are orphaned or zombie.
pub fn find_orphaned_processes() -> Vec<PortInfo> {
    crate::process::find_orphaned_processes()
}

/// Resolve a port number or PID to a killable target.
pub fn resolve_kill_target(n: u32) -> Option<KillTargetResolution> {
    crate::process::resolve_kill_target(n)
}
