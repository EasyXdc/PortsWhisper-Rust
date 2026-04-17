use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessStatus {
    Healthy,
    Orphaned,
    Zombie,
    Unknown,
}

impl ProcessStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Orphaned => "orphaned",
            Self::Zombie => "zombie",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProcessTreeNode {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct PortInfo {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
    pub raw_name: String,
    pub command: String,
    pub cwd: Option<PathBuf>,
    pub project_name: Option<String>,
    pub framework: Option<String>,
    pub uptime: Option<String>,
    pub start_time: Option<String>,
    pub status: ProcessStatus,
    pub memory: Option<String>,
    pub git_branch: Option<String>,
    pub process_tree: Vec<ProcessTreeNode>,
}

#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub process_name: String,
    pub command: String,
    pub description: String,
    pub cpu: f32,
    pub rss_kb: u64,
    pub memory: Option<String>,
    pub cwd: Option<PathBuf>,
    pub project_name: Option<String>,
    pub framework: Option<String>,
    pub uptime: Option<String>,
    pub status_raw: String,
}

#[derive(Clone, Debug)]
pub struct DockerInfo {
    pub host_port: u16,
    pub container_name: String,
    pub image: String,
    pub framework: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LogFdKind {
    Stdout,
    Stderr,
    File,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogFile {
    pub path: PathBuf,
    pub fd: LogFdKind,
    pub kind: String,
    pub priority: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KillResolutionKind {
    Port,
    Pid,
}

#[derive(Clone, Debug)]
pub struct KillTargetResolution {
    pub pid: u32,
    pub via: KillResolutionKind,
    pub port: Option<u16>,
    pub info: Option<PortInfo>,
}

#[derive(Clone, Debug)]
pub struct RawPortEntry {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
}

#[derive(Clone, Debug)]
pub struct RawProcessDetails {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub stat: String,
    pub rss_kb: u64,
    pub lstart: Option<String>,
    pub command: String,
}

#[derive(Clone, Debug)]
pub struct RawProcessEntry {
    pub pid: u32,
    pub process_name: String,
    pub cpu: f32,
    pub mem_percent: f32,
    pub rss_kb: u64,
    pub lstart: Option<String>,
    pub command: String,
}
