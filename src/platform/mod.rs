use crate::error::PortError;
use crate::model::{LogFile, ProcessTreeNode, RawPortEntry, RawProcessDetails, RawProcessEntry};
#[cfg(target_os = "linux")]
use crate::util::command_exists;
#[cfg(unix)]
use crate::util::run_output_with_c_locale;
use crate::util::{basename, run_output};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

pub trait PlatformScanner: Sync {
    fn get_listening_ports_raw(&self) -> Vec<RawPortEntry>;
    fn get_listening_port_raw(&self, port: u16) -> Option<RawPortEntry> {
        self.get_listening_ports_raw()
            .into_iter()
            .find(|entry| entry.port == port)
    }
    fn batch_process_info(&self, pids: &[u32]) -> HashMap<u32, RawProcessDetails>;
    fn get_process_details(&self, pid: u32) -> Option<RawProcessDetails> {
        self.batch_process_info(&[pid]).remove(&pid)
    }
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
    darwin_listening_ports_from_result(run_output(
        "lsof",
        ["-iTCP", "-sTCP:LISTEN", "-P", "-n"],
        Some(Duration::from_millis(10_000)),
    ))
}

fn darwin_listening_ports_from_result(result: Result<String, PortError>) -> Vec<RawPortEntry> {
    darwin_listening_ports_from_output(degrade_command_output(result))
}

fn darwin_listening_ports_from_output(raw: String) -> Vec<RawPortEntry> {
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

fn darwin_listening_port_raw(port: u16) -> Option<RawPortEntry> {
    darwin_listening_ports_from_result(run_output(
        "lsof",
        [&format!("-iTCP:{port}"), "-sTCP:LISTEN", "-P", "-n"],
        Some(Duration::from_millis(10_000)),
    ))
    .into_iter()
    .find(|entry| entry.port == port)
}

#[cfg(target_os = "linux")]
fn linux_listening_ports_raw() -> Vec<RawPortEntry> {
    let proc_entries = linux_listening_ports_from_procfs();
    if !proc_entries.is_empty() {
        return proc_entries;
    }
    let mut entries = Vec::new();
    let mut seen = HashMap::new();
    if command_exists("ss") {
        let raw = degrade_command_output(run_output(
            "ss",
            ["-tlnp"],
            Some(Duration::from_millis(10_000)),
        ));
        if !raw.is_empty() {
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
        let raw = degrade_command_output(run_output(
            "netstat",
            ["-tlnp"],
            Some(Duration::from_millis(10_000)),
        ));
        if !raw.is_empty() {
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

#[cfg(target_os = "linux")]
fn linux_listening_ports_from_procfs() -> Vec<RawPortEntry> {
    let tcp = std::fs::read_to_string("/proc/net/tcp").ok();
    let tcp6 = std::fs::read_to_string("/proc/net/tcp6").ok();
    let pids = std::fs::read_dir("/proc")
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
        .collect::<Vec<_>>();
    linux_listening_ports_from_proc(tcp, tcp6, &pids, linux_pid_socket_inodes)
}

#[cfg(any(test, target_os = "linux"))]
fn linux_listening_ports_from_proc<Lookup>(
    tcp: Option<String>,
    tcp6: Option<String>,
    pids: &[u32],
    inode_lookup: Lookup,
) -> Vec<RawPortEntry>
where
    Lookup: Fn(u32) -> Option<Vec<(u64, String)>>,
{
    let inode_ports = parse_linux_proc_net_tcp(&tcp)
        .into_iter()
        .chain(parse_linux_proc_net_tcp(&tcp6))
        .collect::<HashMap<_, _>>();
    if inode_ports.is_empty() {
        return Vec::new();
    }

    let mut seen = HashMap::new();
    let mut entries = Vec::new();
    for pid in pids {
        let Some(inodes) = inode_lookup(*pid) else {
            continue;
        };
        for (inode, process_name) in inodes {
            let Some(port) = inode_ports.get(&inode) else {
                continue;
            };
            if seen.contains_key(port) {
                continue;
            }
            seen.insert(*port, true);
            entries.push(RawPortEntry {
                port: *port,
                pid: *pid,
                process_name,
            });
        }
    }
    entries.sort_by_key(|entry| entry.port);
    entries
}

#[cfg(any(test, target_os = "linux"))]
fn parse_linux_proc_net_tcp(raw: &Option<String>) -> HashMap<u64, u16> {
    let Some(raw) = raw else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for line in raw.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 || parts[3] != "0A" {
            continue;
        }
        let Some((_, hex_port)) = parts[1].rsplit_once(':') else {
            continue;
        };
        let Ok(port) = u16::from_str_radix(hex_port, 16) else {
            continue;
        };
        let Ok(inode) = parts[9].parse::<u64>() else {
            continue;
        };
        out.insert(inode, port);
    }
    out
}

#[cfg(target_os = "linux")]
fn linux_pid_socket_inodes(pid: u32) -> Option<Vec<(u64, String)>> {
    let fd_dir = std::fs::read_dir(format!("/proc/{pid}/fd")).ok()?;
    let process_name = linux_proc_name(pid);
    let mut out = Vec::new();
    for entry in fd_dir.filter_map(Result::ok) {
        let target = std::fs::read_link(entry.path()).ok()?;
        let label = target.to_string_lossy();
        let Some(inode_text) = label
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))
        else {
            continue;
        };
        let Ok(inode) = inode_text.parse::<u64>() else {
            continue;
        };
        out.push((inode, process_name.clone()));
    }
    Some(out)
}

#[cfg(target_os = "windows")]
fn windows_listening_ports_raw() -> Vec<RawPortEntry> {
    let raw = degrade_command_output(run_output(
        "powershell",
        [
            "-NoProfile",
            "-Command",
            "Get-NetTCPConnection -State Listen | Sort-Object LocalPort | ForEach-Object { $name = (Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).ProcessName; if ($name) { \"{0} {1} {2}\" -f $_.LocalPort, $_.OwningProcess, $name } else { \"{0} {1}\" -f $_.LocalPort, $_.OwningProcess } }",
        ],
        Some(Duration::from_millis(10_000)),
    ));
    let entries = windows_powershell_listening_ports_from_output(raw);
    if !entries.is_empty() {
        return entries;
    }

    let raw = degrade_command_output(run_output(
        "netstat",
        ["-ano", "-p", "TCP"],
        Some(Duration::from_millis(10_000)),
    ));
    windows_netstat_listening_ports_from_output(raw)
}

#[cfg(any(target_os = "windows", test))]
fn windows_powershell_listening_ports_from_output(raw: String) -> Vec<RawPortEntry> {
    let mut entries = Vec::new();
    let mut seen = HashMap::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let Ok(port) = parts[0].parse::<u16>() else {
            continue;
        };
        if seen.contains_key(&port) {
            continue;
        }
        let Ok(pid) = parts[1].parse::<u32>() else {
            continue;
        };
        if pid == 0 {
            continue;
        }
        seen.insert(port, true);
        entries.push(RawPortEntry {
            port,
            pid,
            process_name: if parts.len() > 2 {
                parts[2..].join(" ")
            } else {
                "unknown".to_string()
            },
        });
    }
    entries
}

#[cfg(target_os = "windows")]
fn windows_netstat_listening_ports_from_output(raw: String) -> Vec<RawPortEntry> {
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
    if pids.is_empty() {
        return HashMap::new();
    }
    let pid_list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let result = run_output_with_c_locale(
        "ps",
        [
            "-p",
            &pid_list,
            "-o",
            "pid=,ppid=,stat=,rss=,lstart=,command=",
        ],
        Some(Duration::from_millis(5000)),
    );
    #[cfg(target_os = "linux")]
    {
        return unix_batch_process_info_with(pids, result, |pid| linux_proc_details(*pid));
    }

    #[cfg(not(target_os = "linux"))]
    {
        unix_batch_process_info_with(pids, result, |_| None)
    }
}

fn unix_batch_process_info_with<Lookup>(
    _pids: &[u32],
    result: Result<String, PortError>,
    _linux_fallback: Lookup,
) -> HashMap<u32, RawProcessDetails>
where
    Lookup: Fn(&u32) -> Option<RawProcessDetails>,
{
    let mut map = HashMap::new();
    let raw = degrade_command_output(result);
    for line in raw.lines() {
        if let Some((pid, details)) = parse_unix_ps_details(line) {
            map.insert(pid, details);
        }
    }
    #[cfg(target_os = "linux")]
    {
        for pid in _pids {
            if map.contains_key(pid) {
                continue;
            }
            if let Some(details) = _linux_fallback(pid) {
                map.insert(*pid, details);
            }
        }
    }
    map
}

fn unix_process_details(pid: u32) -> Option<RawProcessDetails> {
    unix_process_details_from_result(run_output_with_c_locale(
        "ps",
        [
            "-p",
            &pid.to_string(),
            "-o",
            "pid=,ppid=,stat=,rss=,lstart=,command=",
        ],
        Some(Duration::from_millis(5000)),
    ))
}

fn unix_process_details_from_result(
    result: Result<String, PortError>,
) -> Option<RawProcessDetails> {
    let raw = degrade_command_output(result);
    raw.lines()
        .find_map(|line| parse_unix_ps_details(line).map(|(_, details)| details))
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
    let raw = run_output(
        "lsof",
        ["-a", "-d", "cwd", "-p", &pid_list],
        Some(Duration::from_millis(10_000)),
    );
    let raw = degrade_command_output(raw);
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
        if let Ok(path) = run_output(
            "powershell",
            [
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid} -ErrorAction SilentlyContinue).Path | Split-Path"),
            ],
            Some(Duration::from_millis(5000)),
        ) {
            if !path.trim().is_empty() {
                map.insert(*pid, PathBuf::from(path));
            }
        }
    }
    map
}

fn unix_all_processes_raw() -> Vec<RawProcessEntry> {
    let raw = degrade_command_output(run_output_with_c_locale(
        "ps",
        ["-eo", "pid=,pcpu=,pmem=,rss=,lstart=,command="],
        Some(Duration::from_millis(5000)),
    ));
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

fn degrade_command_output(result: Result<String, PortError>) -> String {
    result.ok().unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn windows_all_processes_raw() -> Vec<RawProcessEntry> {
    Vec::new()
}

fn unix_process_tree(pid: u32) -> Vec<ProcessTreeNode> {
    unix_process_tree_with(pid, |target_pid| {
        run_output_with_c_locale(
            "ps",
            ["-p", &target_pid.to_string(), "-o", "pid=,ppid=,comm="],
            Some(Duration::from_millis(5000)),
        )
        .ok()
    })
}

fn unix_process_tree_with<Lookup>(pid: u32, lookup: Lookup) -> Vec<ProcessTreeNode>
where
    Lookup: Fn(u32) -> Option<String>,
{
    let mut tree = Vec::new();
    let mut current = pid;
    for _ in 0..8 {
        if current <= 1 {
            break;
        }
        let Some(raw) = lookup(current) else {
            break;
        };
        let line = raw
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            break;
        }
        let Ok(pid) = parts[0].parse::<u32>() else {
            break;
        };
        let ppid = parts[1].parse::<u32>().ok();
        let node = ProcessTreeNode {
            pid,
            ppid,
            name: parts[2..].join(" "),
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
    linux_proc_name_with(pid, |path| std::fs::read_to_string(path))
}

#[cfg(target_os = "linux")]
fn linux_proc_name_with<ReadText>(pid: u32, read_comm: ReadText) -> String
where
    ReadText: Fn(String) -> std::io::Result<String>,
{
    read_comm(format!("/proc/{pid}/comm"))
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
        |path| std::fs::read_to_string(path),
    )
}

#[cfg(target_os = "linux")]
fn linux_proc_details_with<ReadText, ReadStatm, ReadBytes, ReadComm>(
    pid: u32,
    read_stat: ReadText,
    read_statm: ReadStatm,
    read_cmdline: ReadBytes,
    read_comm: ReadComm,
) -> Option<RawProcessDetails>
where
    ReadText: Fn(String) -> std::io::Result<String>,
    ReadStatm: Fn(String) -> std::io::Result<String>,
    ReadBytes: Fn(String) -> std::io::Result<Vec<u8>>,
    ReadComm: Fn(String) -> std::io::Result<String>,
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
        .unwrap_or_else(|| linux_proc_name_with(pid, read_comm));
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
    windows_process_name_with(pid, |target_pid| {
        run_output(
            "powershell",
            [
                "-NoProfile",
                "-Command",
                &format!(
                    "(Get-Process -Id {target_pid} -ErrorAction SilentlyContinue).ProcessName"
                ),
            ],
            Some(Duration::from_millis(3000)),
        )
        .ok()
    })
}

#[cfg(target_os = "windows")]
fn windows_process_name_with<Lookup>(pid: u32, lookup: Lookup) -> Option<String>
where
    Lookup: Fn(u32) -> Option<String>,
{
    let out = lookup(pid)?;
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
    #[cfg(target_os = "linux")]
    use super::unix_batch_process_info_with;
    use super::windows_powershell_listening_ports_from_output;
    #[cfg(target_os = "windows")]
    use super::windows_process_name_with;
    use super::{darwin_listening_ports_from_result, unix_process_details_from_result};
    #[cfg(target_os = "linux")]
    use super::{linux_batch_cwd_with, linux_proc_details_with};
    use super::{linux_listening_ports_from_proc, unix_process_tree_with};
    use crate::error::PortError;
    #[cfg(target_os = "linux")]
    use crate::model::RawProcessDetails;
    #[cfg(target_os = "linux")]
    use std::collections::HashMap;
    #[cfg(target_os = "linux")]
    use std::io::{Error, ErrorKind};
    #[cfg(target_os = "linux")]
    use std::path::PathBuf;

    #[test]
    fn listening_port_scan_timeout_degrades_to_empty_entries() {
        let entries = darwin_listening_ports_from_result(Err(PortError::Timeout {
            cmd: "lsof -iTCP".to_string(),
            ms: 10_000,
        }));

        assert!(entries.is_empty());
    }

    #[test]
    fn unix_process_details_timeout_returns_none() {
        let details = unix_process_details_from_result(Err(PortError::Timeout {
            cmd: "ps -p 42".to_string(),
            ms: 5_000,
        }));

        assert!(details.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unix_batch_process_info_uses_proc_fallback_when_ps_output_times_out() {
        let details = unix_batch_process_info_with(
            &[42],
            Err(PortError::Timeout {
                cmd: "ps -p 42".to_string(),
                ms: 5_000,
            }),
            |pid| {
                Some(RawProcessDetails {
                    pid: *pid,
                    ppid: Some(7),
                    stat: "S".to_string(),
                    rss_kb: 64,
                    lstart: Some("Fri Apr 19 12:00:00 2026".to_string()),
                    command: "fallback-from-proc".to_string(),
                })
            },
        );

        assert_eq!(details.len(), 1);
        assert_eq!(
            details.get(&42).map(|detail| detail.command.as_str()),
            Some("fallback-from-proc")
        );
    }

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
            |_| Err(Error::new(ErrorKind::PermissionDenied, "denied")),
        )
        .expect("linux proc details should still return fallback details");

        assert_eq!(details.pid, 42);
        assert_eq!(details.command, "unknown");
        assert_eq!(details.rss_kb, 0);
        assert_eq!(details.ppid, None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_proc_details_uses_injected_comm_fallback_when_cmdline_is_unreadable() {
        let details = linux_proc_details_with(
            42,
            |_| Err(Error::new(ErrorKind::PermissionDenied, "denied")),
            |_| Err(Error::new(ErrorKind::PermissionDenied, "denied")),
            |_| Err(Error::new(ErrorKind::PermissionDenied, "denied")),
            |_| Ok("from-comm\n".to_string()),
        )
        .expect("linux proc details should still return fallback details");

        assert_eq!(details.command, "from-comm");
        assert_eq!(details.rss_kb, 0);
        assert_eq!(details.ppid, None);
    }

    #[test]
    fn linux_proc_socket_inode_mapping_can_build_listener_entries() {
        let tcp = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  100        0 12345 1 0000000000000000 100 0 0 10 0\n";

        let entries = linux_listening_ports_from_proc(Some(tcp.to_string()), None, &[42], |pid| {
            Some(vec![(12345, "node".to_string())]).filter(|_| pid == 42)
        });

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].port, 3000);
        assert_eq!(entries[0].pid, 42);
        assert_eq!(entries[0].process_name, "node");
    }

    #[test]
    fn unix_process_tree_can_follow_parent_chain_with_targeted_queries() {
        let tree = unix_process_tree_with(42, |pid| match pid {
            42 => Some("42 7 node".to_string()),
            7 => Some("7 1 launchd".to_string()),
            _ => None,
        });

        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].pid, 42);
        assert_eq!(tree[0].ppid, Some(7));
        assert_eq!(tree[0].name, "node");
        assert_eq!(tree[1].pid, 7);
        assert_eq!(tree[1].ppid, Some(1));
        assert_eq!(tree[1].name, "launchd");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn inaccessible_windows_process_returns_none_instead_of_crashing() {
        let name = windows_process_name_with(42, |_| None);
        assert_eq!(name, None);
    }

    #[test]
    fn windows_powershell_listener_output_prefers_first_pid_per_port() {
        let raw = "3000 42 node\n3000 99 duplicate\n8080 7\ninvalid line\n9090 0 system\n";

        let entries = windows_powershell_listening_ports_from_output(raw.to_string());

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].port, 3000);
        assert_eq!(entries[0].pid, 42);
        assert_eq!(entries[0].process_name, "node");
        assert_eq!(entries[1].port, 8080);
        assert_eq!(entries[1].pid, 7);
        assert_eq!(entries[1].process_name, "unknown");
    }
}
