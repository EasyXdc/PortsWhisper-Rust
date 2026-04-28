use super::{
    PlatformScanner, windows_all_processes_raw, windows_batch_cwd, windows_batch_process_info,
    windows_listening_port_raw, windows_listening_ports_raw, windows_process_name,
};
use crate::logs;
use crate::model::{LogFile, ProcessTreeNode, RawPortEntry, RawProcessDetails, RawProcessEntry};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct WindowsScanner;

impl PlatformScanner for WindowsScanner {
    fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
        windows_listening_ports_raw()
    }

    fn get_listening_port_raw(&self, port: u16) -> Option<RawPortEntry> {
        windows_listening_port_raw(port)
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
        crate::kill::execute_kill_command(pid, signal, true)
    }

    fn get_process_log_files(&self, _pid: u32) -> Vec<LogFile> {
        Vec::new()
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
        if follow {
            return None;
        }

        let start_time = match since {
            Some(value) => Some(powershell_start_time_expression(value)?),
            None => None,
        };
        let start_time_clause = start_time
            .map(|expr| format!("; StartTime={expr}"))
            .unwrap_or_default();
        Some(format!(
            "powershell -Command \"Get-WinEvent -FilterHashtable @{{LogName='Application'; ProcessId={pid}{start_time_clause}}} -MaxEvents 50\""
        ))
    }

    fn supports_system_log_follow(&self) -> bool {
        false
    }

    fn build_tail_command(
        &self,
        path: &std::path::Path,
        lines: &str,
        follow: bool,
    ) -> crate::model::TailCommand {
        let escaped_path = path.to_string_lossy().replace('\'', "''");
        let wait = if follow { " -Wait" } else { "" };
        let lines_val = lines.parse::<u32>().unwrap_or(50);
        crate::model::TailCommand::PowerShell {
            command: format!(
                "Get-Content -LiteralPath '{}' -Tail {lines_val}{wait}",
                escaped_path,
            ),
        }
    }

    fn apply_grep_to_shell_command(&self, cmd: &str, grep: &str) -> String {
        let safe_grep = grep.replace('\'', "''");
        format!(
            "powershell -Command \"cmd /C {} | Select-String -SimpleMatch '{}'\"",
            powershell_single_quote(cmd),
            safe_grep
        )
    }

    fn run_shell(&self, cmd: &str) -> i32 {
        use std::process::{Command, Stdio};
        let status = Command::new("cmd")
            .args(["/C", cmd])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();
        if status.map(|s| s.success()).unwrap_or(false) {
            0
        } else {
            1
        }
    }

    fn run_shell_output(&self, cmd: &str) -> Result<String, String> {
        use std::process::{Command, Stdio};
        let output = Command::new("cmd")
            .args(["/C", cmd])
            .stdin(Stdio::null())
            .output()
            .map_err(|err| err.to_string())?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}

fn powershell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn powershell_start_time_expression(value: &str) -> Option<String> {
    if value.starts_with('-') || !crate::model::is_shell_safe(value) {
        return None;
    }

    if let Some(duration) = powershell_relative_duration(value) {
        return Some(duration);
    }

    Some(format!(
        "[datetime]::Parse('{}')",
        value.replace('\'', "''")
    ))
}

fn powershell_relative_duration(value: &str) -> Option<String> {
    let (number, unit) = value.split_at(value.len().checked_sub(1)?);
    let amount = number.parse::<u32>().ok()?;
    let method = match unit {
        "s" => "AddSeconds",
        "m" => "AddMinutes",
        "h" => "AddHours",
        "d" => "AddDays",
        _ => return None,
    };
    Some(format!("(Get-Date).{method}(-{amount})"))
}

#[cfg(test)]
mod tests {
    use super::{PlatformScanner, WindowsScanner};

    #[test]
    fn windows_system_log_since_adds_start_time() {
        let command = WindowsScanner
            .get_system_log_command_with_since(42, false, Some("2h"))
            .expect("relative since should build a command");

        assert!(command.contains("ProcessId=42; StartTime=(Get-Date).AddHours(-2)"));
    }

    #[test]
    fn windows_system_log_follow_is_not_supported() {
        assert!(
            WindowsScanner
                .get_system_log_command_with_since(42, true, None)
                .is_none()
        );
        assert!(!WindowsScanner.supports_system_log_follow());
    }

    #[test]
    fn windows_grep_runs_select_string_inside_powershell() {
        let command = WindowsScanner
            .apply_grep_to_shell_command("powershell -Command \"Get-WinEvent\"", "err");

        assert!(command.starts_with("powershell -Command "));
        assert!(command.contains("cmd /C 'powershell -Command"));
        assert!(command.contains("| Select-String -SimpleMatch 'err'"));
    }
}
