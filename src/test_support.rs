use crate::model::{LogFile, ProcessTreeNode, RawPortEntry, RawProcessDetails, RawProcessEntry};
use crate::platform::PlatformScanner;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

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
