use super::{linux_batch_cwd, linux_listening_ports_raw, unix_all_processes_raw};
use super::{unix_batch_process_info, unix_pid_exists, unix_process_tree, PlatformScanner};
use crate::kill;
use crate::logs;
use crate::model::{LogFile, ProcessTreeNode, RawPortEntry, RawProcessDetails, RawProcessEntry};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct LinuxScanner;

impl PlatformScanner for LinuxScanner {
    fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
        linux_listening_ports_raw()
    }

    fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
        unix_batch_process_info(pids)
    }

    fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf> {
        linux_batch_cwd(pids)
    }

    fn get_all_processes_raw(&self) -> Vec<RawProcessEntry> {
        unix_all_processes_raw()
    }

    fn get_process_tree(&self, pid: u32) -> Vec<ProcessTreeNode> {
        unix_process_tree(pid)
    }

    fn pid_exists(&self, pid: u32) -> bool {
        unix_pid_exists(pid)
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
