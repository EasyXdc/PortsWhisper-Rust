use super::{
    PlatformScanner, unix_batch_process_info, unix_pid_exists, unix_process_details,
    unix_process_tree,
};
use super::{
    darwin_batch_cwd, darwin_listening_port_raw, darwin_listening_ports_raw, unix_all_processes_raw,
};
use crate::logs;
use crate::model::{LogFile, ProcessTreeNode, RawPortEntry, RawProcessDetails, RawProcessEntry};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct MacosScanner;

impl PlatformScanner for MacosScanner {
    fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
        darwin_listening_ports_raw()
    }

    fn get_listening_port_raw(&self, port: u16) -> Option<RawPortEntry> {
        darwin_listening_port_raw(port)
    }

    fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
        unix_batch_process_info(pids)
    }

    fn get_process_details(&self, pid: u32) -> Option<RawProcessDetails> {
        unix_process_details(pid)
    }

    fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf> {
        darwin_batch_cwd(pids)
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
        crate::logs::log_files_from_lsof_result(crate::util::run_output(
            "lsof",
            ["-p", &pid.to_string()],
            Some(std::time::Duration::from_millis(5000)),
        ))
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
        let since_val = since.unwrap_or("1m");
        if !valid_shell_argument_value(since_val) {
            return None;
        }
        Some(if follow {
            format!("log stream --predicate 'processID == {pid}' --style compact")
        } else {
            format!(
                "log show --predicate 'processID == {pid}' --style compact --last {}",
                shell_quote_for_sh(since_val)
            )
        })
    }
}

fn valid_shell_argument_value(value: &str) -> bool {
    !value.starts_with('-') && crate::model::is_shell_safe(value)
}

fn shell_quote_for_sh(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::{MacosScanner, PlatformScanner, shell_quote_for_sh};

    #[test]
    fn macos_system_log_quotes_since_value() {
        let command = MacosScanner
            .get_system_log_command_with_since(42, false, Some("2026-04-28 10:00:00"))
            .expect("safe since value should build a command");

        assert!(command.contains("--last '2026-04-28 10:00:00'"));
    }

    #[test]
    fn macos_system_log_rejects_option_like_since_value() {
        assert!(
            MacosScanner
                .get_system_log_command_with_since(42, false, Some("--predicate"))
                .is_none()
        );
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote_for_sh("a'b"), "'a'\\''b'");
    }
}
