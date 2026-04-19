#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitCode {
    Success,
    Failure,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
        }
    }
}

#[derive(Debug)]
pub enum PortError {
    CommandMissing(String),
    PermissionDenied(String),
    Timeout { cmd: String, ms: u64 },
    ExecFailed { cmd: String, exit: i32, stderr: String },
    ParseFailed { cmd: String, reason: String },
    Io(std::io::Error),
}

impl PortError {
    pub fn short(&self) -> &'static str {
        match self {
            Self::CommandMissing(_) => "command missing",
            Self::PermissionDenied(_) => "permission denied",
            Self::Timeout { .. } => "timeout",
            Self::ExecFailed { .. } => "exec failed",
            Self::ParseFailed { .. } => "parse failed",
            Self::Io(_) => "io error",
        }
    }

    pub fn full(&self) -> String {
        match self {
            Self::CommandMissing(cmd) => format!("{cmd}: not found on PATH"),
            Self::PermissionDenied(cmd) => format!("{cmd}: permission denied"),
            Self::Timeout { cmd, ms } => format!("{cmd}: timeout after {ms}ms"),
            Self::ExecFailed { cmd, exit, stderr } => {
                if stderr.is_empty() {
                    format!("{cmd}: exit {exit}")
                } else {
                    format!("{cmd}: exit {exit}: {stderr}")
                }
            }
            Self::ParseFailed { cmd, reason } => format!("{cmd}: parse failed: {reason}"),
            Self::Io(e) => format!("io: {e}"),
        }
    }
}

impl From<std::io::Error> for PortError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static VERBOSE_ON: AtomicBool = AtomicBool::new(false);
const VERBOSE_CAP: usize = 512;

fn verbose_buf() -> &'static Mutex<Vec<String>> {
    static B: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    B.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn set_verbose_enabled(on: bool) {
    VERBOSE_ON.store(on, Ordering::SeqCst);
}

pub fn verbose_enabled() -> bool {
    VERBOSE_ON.load(Ordering::SeqCst)
}

pub fn verbose_log(msg: impl Into<String>) {
    if !VERBOSE_ON.load(Ordering::SeqCst) {
        return;
    }
    if let Ok(mut buf) = verbose_buf().lock() {
        if buf.len() >= VERBOSE_CAP {
            buf.remove(0);
        }
        buf.push(msg.into());
    }
}

pub fn verbose_log_port_error(e: &PortError) {
    verbose_log(e.full());
}

pub fn drain_verbose_log() -> Vec<String> {
    verbose_buf()
        .lock()
        .map(|mut buf| std::mem::take(&mut *buf))
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) fn verbose_test_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_missing_short_and_full_are_distinguishable() {
        let e = PortError::CommandMissing("lsof".into());
        assert_eq!(e.short(), "command missing");
        assert!(e.full().contains("lsof"));
        assert!(e.full().contains("not found"));
    }

    #[test]
    fn permission_denied_mentions_cmd() {
        let e = PortError::PermissionDenied("lsof".into());
        assert_eq!(e.short(), "permission denied");
        assert!(e.full().contains("lsof"));
        assert!(e.full().contains("permission"));
    }

    #[test]
    fn timeout_reports_cmd_and_ms() {
        let e = PortError::Timeout {
            cmd: "lsof -iTCP".into(),
            ms: 10_000,
        };
        assert_eq!(e.short(), "timeout");
        let full = e.full();
        assert!(full.contains("lsof -iTCP"));
        assert!(full.contains("10000"));
    }

    #[test]
    fn exec_failed_carries_exit_and_stderr() {
        let e = PortError::ExecFailed {
            cmd: "ps".into(),
            exit: 2,
            stderr: "bad flag".into(),
        };
        assert_eq!(e.short(), "exec failed");
        let full = e.full();
        assert!(full.contains("ps"));
        assert!(full.contains("exit 2"));
        assert!(full.contains("bad flag"));
    }

    #[test]
    fn parse_failed_reports_cmd_and_reason() {
        let e = PortError::ParseFailed {
            cmd: "ps".into(),
            reason: "unexpected column count".into(),
        };
        assert_eq!(e.short(), "parse failed");
        let full = e.full();
        assert!(full.contains("ps"));
        assert!(full.contains("unexpected column count"));
    }

    #[test]
    fn io_wraps_std_io_error() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let e = PortError::Io(io);
        assert_eq!(e.short(), "io error");
        assert!(e.full().contains("no such file"));
    }

    #[test]
    fn verbose_log_ignores_messages_when_disabled() {
        let _guard = verbose_test_lock().lock().unwrap();
        set_verbose_enabled(false);
        drain_verbose_log();

        verbose_log("ignored");
        assert!(drain_verbose_log().is_empty());
    }

    #[test]
    fn verbose_log_captures_messages_when_enabled() {
        let _guard = verbose_test_lock().lock().unwrap();
        set_verbose_enabled(true);
        drain_verbose_log();

        verbose_log("first");
        verbose_log_port_error(&PortError::CommandMissing("lsof".into()));

        let drained = drain_verbose_log();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], "first");
        assert!(drained[1].contains("lsof"));
        assert!(drained[1].contains("not found"));

        assert!(drain_verbose_log().is_empty());
        set_verbose_enabled(false);
    }

    #[test]
    fn verbose_log_ring_buffer_drops_oldest_beyond_cap() {
        let _guard = verbose_test_lock().lock().unwrap();
        set_verbose_enabled(true);
        drain_verbose_log();

        for i in 0..(VERBOSE_CAP + 10) {
            verbose_log(format!("msg-{i}"));
        }
        let drained = drain_verbose_log();
        assert_eq!(drained.len(), VERBOSE_CAP);
        assert_eq!(drained.first().map(String::as_str), Some("msg-10"));
        assert_eq!(
            drained.last().map(String::as_str),
            Some(format!("msg-{}", VERBOSE_CAP + 9)).as_deref()
        );
        set_verbose_enabled(false);
    }
}
