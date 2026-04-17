use super::{
    PlatformScanner, windows_all_processes_raw, windows_batch_cwd, windows_batch_process_info,
    windows_listening_ports_raw, windows_process_name,
};
use crate::kill;
use crate::logs;
use crate::model::{LogFile, ProcessTreeNode, RawPortEntry, RawProcessDetails, RawProcessEntry};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct WindowsScanner;

impl PlatformScanner for WindowsScanner {
    fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
        windows_listening_ports_raw()
    }

    fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
        windows_batch_process_info(pids)
    }

    fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf> {
        windows_batch_cwd(pids)
    }

    fn get_all_processes_raw(&self) -> Vec<RawProcessEntry> {
        windows_all_processes_raw()
    }

    fn get_process_tree(&self, _pid: u32) -> Vec<ProcessTreeNode> {
        Vec::new()
    }

    fn pid_exists(&self, pid: u32) -> bool {
        windows_process_name(pid).is_some()
    }

    fn kill_process(&self, pid: u32, signal: &str) -> bool {
        kill::kill_process(pid, signal)
    }

    fn get_process_log_files(&self, pid: u32) -> Vec<LogFile> {
        logs::get_process_log_files(pid)
    }

    fn get_system_log_command(&self, pid: u32, follow: bool) -> Option<String> {
        logs::get_system_log_command(pid, follow)
    }
}
