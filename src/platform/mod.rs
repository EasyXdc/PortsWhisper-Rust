use crate::model::{LogFile, ProcessTreeNode, RawPortEntry, RawProcessDetails, RawProcessEntry};
#[cfg(target_os = "linux")]
use crate::util::command_exists;
use crate::util::{basename, run_output};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

pub trait PlatformScanner {
    fn get_listening_ports_raw(&self) -> Vec<RawPortEntry>;
    fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails>;
    fn batch_cwd(&self, pids: &[u32]) -> HashMap<u32, PathBuf>;
    fn get_all_processes_raw(&self) -> Vec<RawProcessEntry>;
    fn get_process_tree(&self, pid: u32) -> Vec<ProcessTreeNode>;
    fn pid_exists(&self, pid: u32) -> bool;
    fn kill_process(&self, pid: u32, signal: &str) -> bool;
    fn get_process_log_files(&self, pid: u32) -> Vec<LogFile>;
    fn get_system_log_command(&self, pid: u32, follow: bool) -> Option<String>;
}

pub fn listening_ports_raw() -> Vec<RawPortEntry> {
    native_scanner().get_listening_ports_raw()
}

pub fn batch_process_info(pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
    native_scanner().batch_process_info(pids)
}

pub fn batch_cwd(pids: &[u32]) -> HashMap<u32, PathBuf> {
    native_scanner().batch_cwd(pids)
}

pub fn all_processes_raw() -> Vec<RawProcessEntry> {
    native_scanner().get_all_processes_raw()
}

pub fn process_tree(pid: u32) -> Vec<ProcessTreeNode> {
    native_scanner().get_process_tree(pid)
}

pub fn pid_exists(pid: u32) -> bool {
    native_scanner().pid_exists(pid)
}

pub(crate) fn native_scanner() -> &'static dyn PlatformScanner {
    #[cfg(target_os = "macos")]
    {
        &macos::MacosScanner
    }
    #[cfg(target_os = "linux")]
    {
        &linux::LinuxScanner
    }
    #[cfg(target_os = "windows")]
    {
        &windows::WindowsScanner
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        &UnsupportedScanner
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
struct UnsupportedScanner;

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
impl PlatformScanner for UnsupportedScanner {
    fn get_listening_ports_raw(&self) -> Vec<RawPortEntry> {
        Vec::new()
    }

    fn batch_process_info(&self, _pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
        HashMap::new()
    }

    fn batch_cwd(&self, _pids: &[u32]) -> HashMap<u32, PathBuf> {
        HashMap::new()
    }

    fn get_all_processes_raw(&self) -> Vec<RawProcessEntry> {
        Vec::new()
    }

    fn get_process_tree(&self, _pid: u32) -> Vec<ProcessTreeNode> {
        Vec::new()
    }

    fn pid_exists(&self, _pid: u32) -> bool {
        false
    }

    fn kill_process(&self, _pid: u32, _signal: &str) -> bool {
        false
    }

    fn get_process_log_files(&self, _pid: u32) -> Vec<LogFile> {
        Vec::new()
    }

    fn get_system_log_command(&self, _pid: u32, _follow: bool) -> Option<String> {
        None
    }
}

fn darwin_listening_ports_raw() -> Vec<RawPortEntry> {
    let raw =
        run_output("lsof", ["-iTCP", "-sTCP:LISTEN", "-P", "-n"], Some(10_000)).unwrap_or_default();
    let mut entries = Vec::new();
    let mut seen = HashMap::new();
    for line in raw.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }
        let Some(port) = parse_port_suffix(parts[8]) else {
            continue;
        };
        if seen.contains_key(&port) {
            continue;
        }
        let Ok(pid) = parts[1].parse::<u32>() else {
            continue;
        };
        seen.insert(port, true);
        entries.push(RawPortEntry {
            port,
            pid,
            process_name: parts[0].to_string(),
        });
    }
    entries
}

#[cfg(target_os = "linux")]
fn linux_listening_ports_raw() -> Vec<RawPortEntry> {
    let mut entries = Vec::new();
    let mut seen = HashMap::new();
    if command_exists("ss") {
        if let Some(raw) = run_output("ss", ["-tlnp"], Some(10_000)) {
            for line in raw.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 5 {
                    continue;
                }
                let Some(port) = parse_port_suffix(parts[3]) else {
                    continue;
                };
                if seen.contains_key(&port) {
                    continue;
                }
                let users = parts.get(5..).unwrap_or(&[]).join(" ");
                let Some(pid) = parse_after(&users, "pid=", |c| !c.is_ascii_digit()) else {
                    continue;
                };
                let process_name =
                    parse_quoted_process_name(&users).unwrap_or_else(|| linux_proc_name(pid));
                seen.insert(port, true);
                entries.push(RawPortEntry {
                    port,
                    pid,
                    process_name,
                });
            }
        }
    }

    if entries.is_empty() && command_exists("netstat") {
        if let Some(raw) = run_output("netstat", ["-tlnp"], Some(10_000)) {
            for line in raw.lines() {
                if !line.contains("LISTEN") {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 7 {
                    continue;
                }
                let Some(port) = parse_port_suffix(parts[3]) else {
                    continue;
                };
                if seen.contains_key(&port) {
                    continue;
                }
                let pid_program = parts[parts.len() - 1];
                let pair: Vec<&str> = pid_program.splitn(2, '/').collect();
                if pair.len() != 2 {
                    continue;
                }
                let Ok(pid) = pair[0].parse::<u32>() else {
                    continue;
                };
                seen.insert(port, true);
                entries.push(RawPortEntry {
                    port,
                    pid,
                    process_name: pair[1].to_string(),
                });
            }
        }
    }
    entries
}

#[cfg(target_os = "windows")]
fn windows_listening_ports_raw() -> Vec<RawPortEntry> {
    let raw = run_output("netstat", ["-ano", "-p", "TCP"], Some(10_000)).unwrap_or_default();
    let mut entries = Vec::new();
    let mut seen = HashMap::new();
    for line in raw.lines().filter(|l| l.contains("LISTENING")) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        let Some(port) = parse_port_suffix(parts[1]) else {
            continue;
        };
        if seen.contains_key(&port) {
            continue;
        }
        let Ok(pid) = parts[parts.len() - 1].parse::<u32>() else {
            continue;
        };
        if pid == 0 {
            continue;
        }
        seen.insert(port, true);
        entries.push(RawPortEntry {
            port,
            pid,
            process_name: windows_process_name(pid).unwrap_or_else(|| "unknown".to_string()),
        });
    }
    entries
}

fn unix_batch_process_info(pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }
    let pid_list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let raw = run_output(
        "ps",
        [
            "-p",
            &pid_list,
            "-o",
            "pid=,ppid=,stat=,rss=,lstart=,command=",
        ],
        Some(5000),
    )
    .unwrap_or_default();
    for line in raw.lines() {
        if let Some((pid, details)) = parse_unix_ps_details(line) {
            map.insert(pid, details);
        }
    }
    #[cfg(target_os = "linux")]
    {
        for pid in pids {
            if map.contains_key(pid) {
                continue;
            }
            if let Some(details) = linux_proc_details(*pid) {
                map.insert(*pid, details);
            }
        }
    }
    map
}

#[cfg(target_os = "windows")]
fn windows_batch_process_info(pids: &[u32]) -> HashMap<u32, RawProcessDetails> {
    let mut map = HashMap::new();
    for pid in pids {
        let name = windows_process_name(*pid).unwrap_or_else(|| "unknown".to_string());
        map.insert(
            *pid,
            RawProcessDetails {
                pid: *pid,
                ppid: None,
                stat: "S".to_string(),
                rss_kb: 0,
                lstart: None,
                command: name,
            },
        );
    }
    map
}

fn darwin_batch_cwd(pids: &[u32]) -> HashMap<u32, PathBuf> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }
    let pid_list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let raw =
        run_output("lsof", ["-a", "-d", "cwd", "-p", &pid_list], Some(10_000)).unwrap_or_default();
    for line in raw.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }
        if let Ok(pid) = parts[1].parse::<u32>() {
            let path = parts[8..].join(" ");
            if path.starts_with('/') {
                map.insert(pid, PathBuf::from(path));
            }
        }
    }
    map
}

#[cfg(target_os = "linux")]
fn linux_batch_cwd(pids: &[u32]) -> HashMap<u32, PathBuf> {
    linux_batch_cwd_with(pids, |path| std::fs::read_link(path))
}

#[cfg(target_os = "linux")]
fn linux_batch_cwd_with<ReadLink>(pids: &[u32], read_link: ReadLink) -> HashMap<u32, PathBuf>
where
    ReadLink: Fn(&PathBuf) -> std::io::Result<PathBuf>,
{
    let mut map = HashMap::new();
    for pid in pids {
        let path = PathBuf::from(format!("/proc/{pid}/cwd"));
        if let Ok(target) = read_link(&path) {
            if target.is_absolute() {
                map.insert(*pid, target);
            }
        }
    }
    map
}

#[cfg(target_os = "windows")]
fn windows_batch_cwd(pids: &[u32]) -> HashMap<u32, PathBuf> {
    let mut map = HashMap::new();
    for pid in pids {
        if let Some(path) = run_output(
            "powershell",
            [
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid} -ErrorAction SilentlyContinue).Path | Split-Path"),
            ],
            Some(5000),
        ) {
            if !path.trim().is_empty() {
                map.insert(*pid, PathBuf::from(path));
            }
        }
    }
    map
}

fn unix_all_processes_raw() -> Vec<RawProcessEntry> {
    let raw = run_output(
        "ps",
        ["-eo", "pid=,pcpu=,pmem=,rss=,lstart=,command="],
        Some(5000),
    )
    .unwrap_or_default();
    let current_pid = std::process::id();
    let mut entries = Vec::new();
    let mut seen = HashMap::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }
        let Ok(pid) = parts[0].parse::<u32>() else {
            continue;
        };
        if pid <= 1 || pid == current_pid || seen.contains_key(&pid) {
            continue;
        }
        let cpu = parts[1].parse::<f32>().unwrap_or(0.0);
        let mem_percent = parts[2].parse::<f32>().unwrap_or(0.0);
        let rss_kb = parts[3].parse::<u64>().unwrap_or(0);
        let lstart = Some(parts[4..9].join(" "));
        let command = parts[9..].join(" ");
        let process_name = basename(command.split_whitespace().next().unwrap_or("unknown"));
        seen.insert(pid, true);
        entries.push(RawProcessEntry {
            pid,
            process_name,
            cpu,
            mem_percent,
            rss_kb,
            lstart,
            command,
        });
    }
    entries
}

#[cfg(target_os = "windows")]
fn windows_all_processes_raw() -> Vec<RawProcessEntry> {
    Vec::new()
}

fn unix_process_tree(pid: u32) -> Vec<ProcessTreeNode> {
    let raw = run_output("ps", ["-eo", "pid=,ppid=,comm="], Some(5000)).unwrap_or_default();
    let mut processes = HashMap::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let Ok(p) = parts[0].parse::<u32>() else {
            continue;
        };
        let ppid = parts[1].parse::<u32>().ok();
        processes.insert(
            p,
            ProcessTreeNode {
                pid: p,
                ppid,
                name: parts[2..].join(" "),
            },
        );
    }
    let mut tree = Vec::new();
    let mut current = pid;
    for _ in 0..8 {
        if current <= 1 {
            break;
        }
        let Some(node) = processes.get(&current).cloned() else {
            break;
        };
        current = node.ppid.unwrap_or(0);
        tree.push(node);
    }
    tree
}

fn parse_unix_ps_details(line: &str) -> Option<(u32, RawProcessDetails)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 9 {
        return None;
    }
    let pid = parts[0].parse().ok()?;
    let ppid = parts[1].parse().ok();
    let stat = parts[2].to_string();
    let rss_kb = parts[3].parse().unwrap_or(0);
    let lstart = Some(parts[4..9].join(" "));
    let command = parts[9..].join(" ");
    Some((
        pid,
        RawProcessDetails {
            pid,
            ppid,
            stat,
            rss_kb,
            lstart,
            command,
        },
    ))
}

#[cfg(target_os = "linux")]
fn linux_proc_name(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(target_os = "linux")]
fn linux_proc_details(pid: u32) -> Option<RawProcessDetails> {
    linux_proc_details_with(
        pid,
        |path| std::fs::read_to_string(path),
        |path| std::fs::read_to_string(path),
        |path| std::fs::read(path),
    )
}

#[cfg(target_os = "linux")]
fn linux_proc_details_with<ReadText, ReadStatm, ReadBytes>(
    pid: u32,
    read_stat: ReadText,
    read_statm: ReadStatm,
    read_cmdline: ReadBytes,
) -> Option<RawProcessDetails>
where
    ReadText: Fn(String) -> std::io::Result<String>,
    ReadStatm: Fn(String) -> std::io::Result<String>,
    ReadBytes: Fn(String) -> std::io::Result<Vec<u8>>,
{
    let stat_content = read_stat(format!("/proc/{pid}/stat")).ok();
    let (stat, ppid) = if let Some(stat_content) = stat_content {
        let close = stat_content.rfind(')')?;
        let after: Vec<&str> = stat_content[close + 2..].split_whitespace().collect();
        (
            after.first().copied().unwrap_or("?").to_string(),
            after.get(1).and_then(|v| v.parse().ok()),
        )
    } else {
        ("?".to_string(), None)
    };

    let rss_kb = read_statm(format!("/proc/{pid}/statm"))
        .ok()
        .and_then(|v| {
            v.split_whitespace()
                .nth(1)
                .and_then(|n| n.parse::<u64>().ok())
        })
        .unwrap_or(0)
        * 4;
    let command = read_cmdline(format!("/proc/{pid}/cmdline"))
        .ok()
        .map(|bytes| {
            bytes
                .split(|b| *b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| linux_proc_name(pid));
    Some(RawProcessDetails {
        pid,
        ppid,
        stat,
        rss_kb,
        lstart: None,
        command,
    })
}

#[cfg(target_os = "windows")]
fn windows_process_name(pid: u32) -> Option<String> {
    let out = run_output(
        "powershell",
        [
            "-NoProfile",
            "-Command",
            &format!("(Get-Process -Id {pid} -ErrorAction SilentlyContinue).ProcessName"),
        ],
        Some(3000),
    )?;
    if out.is_empty() {
        None
    } else {
        Some(out.trim_end_matches(".exe").to_string())
    }
}

fn parse_port_suffix(addr: &str) -> Option<u16> {
    let idx = addr.rfind(':')?;
    addr[idx + 1..].parse().ok()
}

#[cfg(target_os = "linux")]
fn parse_after<F>(s: &str, marker: &str, stop: F) -> Option<u32>
where
    F: Fn(char) -> bool,
{
    let start = s.find(marker)? + marker.len();
    let mut out = String::new();
    for ch in s[start..].chars() {
        if stop(ch) {
            break;
        }
        out.push(ch);
    }
    out.parse().ok()
}

#[cfg(target_os = "linux")]
fn parse_quoted_process_name(s: &str) -> Option<String> {
    let start = s.find("(\"")? + 2;
    let end = s[start..].find('"')?;
    Some(s[start..start + end].to_string())
}

fn unix_pid_exists(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "windows")]
    use super::windows_process_name_with;
    #[cfg(target_os = "linux")]
    use super::{linux_batch_cwd_with, linux_proc_details_with};
    #[cfg(target_os = "linux")]
    use crate::model::RawProcessDetails;
    #[cfg(target_os = "linux")]
    use std::collections::HashMap;
    #[cfg(target_os = "linux")]
    use std::io::{Error, ErrorKind};
    #[cfg(target_os = "linux")]
    use std::path::PathBuf;

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_permission_errors_return_partial_defaults_instead_of_crashing() {
        let cwd_map = linux_batch_cwd_with(&[42], |_| {
            Err(Error::new(ErrorKind::PermissionDenied, "denied"))
        });
        assert!(cwd_map.is_empty());

        let details = linux_proc_details_with(
            42,
            |_| Err(Error::new(ErrorKind::PermissionDenied, "denied")),
            |_| Err(Error::new(ErrorKind::PermissionDenied, "denied")),
            |_| Err(Error::new(ErrorKind::PermissionDenied, "denied")),
        )
        .expect("linux proc details should still return fallback details");

        assert_eq!(details.pid, 42);
        assert_eq!(details.command, "unknown");
        assert_eq!(details.rss_kb, 0);
        assert_eq!(details.ppid, None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn inaccessible_windows_process_returns_none_instead_of_crashing() {
        let name = windows_process_name_with(42, |_| None);
        assert_eq!(name, None);
    }
}
