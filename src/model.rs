use std::path::PathBuf;
use std::{fmt, str::FromStr};

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
pub struct DisplayTime {
    pub weekday: String,
    pub month_name: String,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub year: u16,
}

impl fmt::Display for DisplayTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {:02} {:02}:{:02}:{:02} {}",
            self.weekday, self.month_name, self.day, self.hour, self.minute, self.second, self.year
        )
    }
}

impl FromStr for DisplayTime {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(());
        }
        let time: Vec<&str> = parts[3].split(':').collect();
        if time.len() != 3 {
            return Err(());
        }
        Ok(Self {
            weekday: parts[0].to_string(),
            month_name: parts[1].to_string(),
            month: month_number(parts[1])?,
            day: parts[2].parse().map_err(|_| ())?,
            hour: time[0].parse().map_err(|_| ())?,
            minute: time[1].parse().map_err(|_| ())?,
            second: time[2].parse().map_err(|_| ())?,
            year: parts[4].parse().map_err(|_| ())?,
        })
    }
}

fn month_number(name: &str) -> Result<u8, ()> {
    match name {
        "Jan" => Ok(1),
        "Feb" => Ok(2),
        "Mar" => Ok(3),
        "Apr" => Ok(4),
        "May" => Ok(5),
        "Jun" => Ok(6),
        "Jul" => Ok(7),
        "Aug" => Ok(8),
        "Sep" => Ok(9),
        "Oct" => Ok(10),
        "Nov" => Ok(11),
        "Dec" => Ok(12),
        _ => Err(()),
    }
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
    pub start_time: Option<DisplayTime>,
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
