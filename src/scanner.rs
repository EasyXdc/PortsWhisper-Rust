use crate::model::{KillTargetResolution, PortInfo, ProcessInfo};

pub use crate::framework::{
    detect_framework, detect_framework_from_command, detect_framework_from_image,
    detect_framework_from_name, summarize_command,
};
pub use crate::framework::{is_dev_process, is_docker_process};

pub fn get_listening_ports(detailed: bool) -> Vec<PortInfo> {
    crate::ports::get_listening_ports(detailed)
}

pub fn get_port_details(port: u16) -> Option<PortInfo> {
    crate::ports::get_port_details(port)
}

pub fn get_all_processes() -> Vec<ProcessInfo> {
    crate::process::get_all_processes()
}

pub fn get_all_dev_processes() -> Vec<ProcessInfo> {
    crate::process::get_all_dev_processes()
}

pub fn find_orphaned_processes() -> Vec<PortInfo> {
    crate::process::find_orphaned_processes()
}

pub fn resolve_kill_target(n: u32) -> Option<KillTargetResolution> {
    crate::process::resolve_kill_target(n)
}
