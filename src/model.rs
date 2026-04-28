use std::path::PathBuf;
use std::{fmt, str::FromStr};

/// Maximum valid TCP port number.
pub const MAX_PORT: u32 = 65_535;

/// Returns true if `n` is in the valid TCP port range (1–65535).
pub fn is_likely_port(n: u32) -> bool {
    (1..=MAX_PORT).contains(&n)
}

/// Returns true if `s` contains only shell-safe characters (no `$`, backticks, `!`, `;`, `|`, `&`, parentheses, or newlines).
pub fn is_shell_safe(s: &str) -> bool {
    !s.chars().any(|c| {
        matches!(
            c,
            '$' | '`'
                | '!'
                | ';'
                | '|'
                | '&'
                | '('
                | ')'
                | '<'
                | '>'
                | '"'
                | '\\'
                | '%'
                | '\n'
                | '\r'
        )
    })
}

/// Runtime health classification for a listener process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessStatus {
    /// Process has a known parent and is functioning normally.
    Healthy,
    /// Process parent is PID 1 (likely detached from terminal).
    Orphaned,
    /// Process is in zombie state (defunct, awaiting reaping).
    Zombie,
    /// Process status could not be determined.
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

/// A single node in a process ancestry chain.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessTreeNode {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
}

/// Parsed representation of a process start timestamp.
#[derive(Clone, Debug, PartialEq)]
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

pub(crate) fn month_number(name: &str) -> Result<u8, ()> {
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

/// Complete metadata for a process listening on a TCP port.
#[derive(Clone, Debug, PartialEq)]
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

/// Metadata for a running process in the dev-process table.
#[derive(Clone, Debug, PartialEq)]
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

/// Container-to-host-port mapping discovered from `docker ps`.
#[derive(Clone, Debug)]
pub struct DockerInfo {
    pub host_port: u16,
    pub container_name: String,
    pub image: String,
    pub framework: String,
}

/// Classification of a log file descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LogFdKind {
    Stdout,
    Stderr,
    File,
}

/// A discovered log file associated with a process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogFile {
    pub path: PathBuf,
    pub fd: LogFdKind,
    pub kind: String,
    pub priority: u8,
}

/// Whether a kill target was resolved by port or by PID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KillResolutionKind {
    Port,
    Pid,
}

/// Result of resolving a kill target from a port or PID number.
#[derive(Clone, Debug)]
pub struct KillTargetResolution {
    pub pid: u32,
    pub via: KillResolutionKind,
    pub port: Option<u16>,
    pub info: Option<PortInfo>,
}

/// Minimal data returned by the platform port scanner before enrichment.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RawPortEntry {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
}

/// Process details obtained from `ps` or `/proc` before enrichment.
#[derive(Clone, Debug)]
pub struct RawProcessDetails {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub stat: String,
    pub rss_kb: u64,
    pub lstart: Option<String>,
    pub command: String,
}

/// Raw process list entry from `ps -eo` before enrichment.
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TailCommand {
    PowerShell { command: String },
    Argv(Vec<String>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_status_labels_match_conventions() {
        assert_eq!(ProcessStatus::Healthy.label(), "healthy");
        assert_eq!(ProcessStatus::Orphaned.label(), "orphaned");
        assert_eq!(ProcessStatus::Zombie.label(), "zombie");
        assert_eq!(ProcessStatus::Unknown.label(), "unknown");
    }

    #[test]
    fn display_time_parses_standard_lstart_format() {
        let dt: DisplayTime = "Fri Apr 17 10:00:00 2026".parse().unwrap();
        assert_eq!(dt.weekday, "Fri");
        assert_eq!(dt.month_name, "Apr");
        assert_eq!(dt.month, 4);
        assert_eq!(dt.day, 17);
        assert_eq!(dt.hour, 10);
        assert_eq!(dt.minute, 0);
        assert_eq!(dt.second, 0);
        assert_eq!(dt.year, 2026);
    }

    #[test]
    fn display_time_rejects_malformed_input() {
        assert!("".parse::<DisplayTime>().is_err());
        assert!("invalid".parse::<DisplayTime>().is_err());
        assert!("Fri Apr 17 10:00 2026".parse::<DisplayTime>().is_err());
    }

    #[test]
    fn display_time_formats_back_to_original() {
        let original = "Fri Apr 17 10:00:00 2026";
        let dt: DisplayTime = original.parse().unwrap();
        assert_eq!(dt.to_string(), original);
    }

    #[test]
    fn month_number_maps_all_abbreviations() {
        assert_eq!(month_number("Jan"), Ok(1));
        assert_eq!(month_number("Feb"), Ok(2));
        assert_eq!(month_number("Mar"), Ok(3));
        assert_eq!(month_number("Apr"), Ok(4));
        assert_eq!(month_number("May"), Ok(5));
        assert_eq!(month_number("Jun"), Ok(6));
        assert_eq!(month_number("Jul"), Ok(7));
        assert_eq!(month_number("Aug"), Ok(8));
        assert_eq!(month_number("Sep"), Ok(9));
        assert_eq!(month_number("Oct"), Ok(10));
        assert_eq!(month_number("Nov"), Ok(11));
        assert_eq!(month_number("Dec"), Ok(12));
        assert_eq!(month_number("Foo"), Err(()));
    }

    #[test]
    fn is_likely_port_rejects_zero() {
        assert!(!is_likely_port(0));
        assert!(is_likely_port(1));
        assert!(is_likely_port(80));
        assert!(is_likely_port(65_535));
        assert!(!is_likely_port(65_536));
    }

    #[test]
    fn is_shell_safe_rejects_metacharacters() {
        assert!(is_shell_safe("1h"));
        assert!(is_shell_safe("2024-01-01 10:00:00"));
        assert!(!is_shell_safe("$(rm -rf /)"));
        assert!(!is_shell_safe("`whoami`"));
        assert!(!is_shell_safe("1h; rm -rf /"));
        assert!(!is_shell_safe("1h|cat /etc/passwd"));
        assert!(!is_shell_safe("$HOME"));
        assert!(!is_shell_safe("test\"injection"));
        assert!(!is_shell_safe("path\\with\\backslash"));
        assert!(!is_shell_safe("100%"));
    }
}
