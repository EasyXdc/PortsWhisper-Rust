use super::{
    PlatformScanner, unix_batch_process_info, unix_pid_exists, unix_process_details,
    unix_process_tree,
};
use super::{
    linux_batch_cwd, linux_listening_port_raw, linux_listening_ports_raw, unix_all_processes_raw,
};
use crate::logs;
use crate::model::{LogFile, ProcessTreeNode, RawPortEntry, RawProcessDetails, RawProcessEntry};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct LinuxScanner;

impl PlatformScanner for LinuxScanner {
    fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
        linux_listening_ports_raw()
    }

    fn get_listening_port_raw(&self, port: u16) -> Option<RawPortEntry> {
        linux_listening_port_raw(port)
    }

    fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
        unix_batch_process_info(pids)
    }

    fn get_process_details(&self, pid: u32) -> Option<RawProcessDetails> {
        unix_process_details(pid)
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
        crate::kill::execute_kill_command(pid, signal, false)
    }

    fn get_process_log_files(&self, pid: u32) -> Vec<LogFile> {
        let proc_files = crate::logs::log_files_from_proc_fd(pid);
        let lsof_files = crate::logs::log_files_from_lsof_result(crate::util::run_output(
            "lsof",
            ["-p", &pid.to_string()],
            Some(std::time::Duration::from_millis(5000)),
        ));
        crate::logs::merge_log_discovery_results(proc_files, lsof_files)
    }

    fn get_system_log_command(&self, pid: u32, follow: bool) -> Option<String> {
        logs::get_system_log_command(pid, follow)
    }

    fn get_system_log_command_with_since(
        &self,
        pid: u32,
        follow: bool,
        since: Option<&str>,
    ) -> Option<String> {
        if let Some(since_val) = since
            && (since_val.starts_with('-') || !crate::model::is_shell_safe(since_val))
        {
            return None;
        }
        Some(if follow {
            match since {
                Some(since) => format!("journalctl _PID={pid} --since {since:?} -f --no-pager"),
                None => format!("journalctl _PID={pid} -f --no-pager"),
            }
        } else {
            match since {
                Some(since) => format!("journalctl _PID={pid} --since {since:?} --no-pager -n 50"),
                None => format!("journalctl _PID={pid} --no-pager -n 50"),
            }
        })
    }
}
