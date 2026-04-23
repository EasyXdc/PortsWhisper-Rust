#[derive(Debug)]
pub enum PortError {
    CommandMissing(String),
    PermissionDenied(String),
    Timeout {
        cmd: String,
        ms: u64,
    },
    ExecFailed {
        cmd: String,
        exit: i32,
        stderr: String,
    },
    ParseFailed {
        cmd: String,
        reason: String,
    },
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

    pub fn user_message(&self) -> String {
        match self {
            Self::CommandMissing(cmd) => format!("{cmd} not found; results may be incomplete"),
            Self::PermissionDenied(_) => "permission denied while inspecting processes".into(),
            Self::Timeout { .. } => "system command timed out; results may be incomplete".into(),
            Self::ParseFailed { .. } => "system process output could not be parsed".into(),
            Self::ExecFailed { .. } | Self::Io(_) => "system inspection failed".into(),
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

fn user_warning_buf() -> &'static Mutex<Vec<PortError>> {
    static B: OnceLock<Mutex<Vec<PortError>>> = OnceLock::new();
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

pub fn record_user_warning(e: &PortError) {
    if let Ok(mut buf) = user_warning_buf().lock() {
        buf.push(match e {
            PortError::CommandMissing(cmd) => PortError::CommandMissing(cmd.clone()),
            PortError::PermissionDenied(cmd) => PortError::PermissionDenied(cmd.clone()),
            PortError::Timeout { cmd, ms } => PortError::Timeout {
                cmd: cmd.clone(),
                ms: *ms,
            },
            PortError::ExecFailed { cmd, exit, stderr } => PortError::ExecFailed {
                cmd: cmd.clone(),
                exit: *exit,
                stderr: stderr.clone(),
            },
            PortError::ParseFailed { cmd, reason } => PortError::ParseFailed {
                cmd: cmd.clone(),
                reason: reason.clone(),
            },
            PortError::Io(io) => PortError::Io(std::io::Error::new(io.kind(), io.to_string())),
        });
    }
}

pub fn drain_user_warnings() -> Vec<PortError> {
    user_warning_buf()
        .lock()
        .map(|mut buf| std::mem::take(&mut *buf))
        .unwrap_or_default()
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
    fn user_message_summarizes_user_facing_errors() {
        assert_eq!(
            PortError::CommandMissing("lsof".into()).user_message(),
            "lsof not found; results may be incomplete"
        );
        assert_eq!(
            PortError::PermissionDenied("lsof".into()).user_message(),
            "permission denied while inspecting processes"
        );
        assert_eq!(
            PortError::Timeout {
                cmd: "lsof -iTCP".into(),
                ms: 10_000,
            }
            .user_message(),
            "system command timed out; results may be incomplete"
        );
        assert_eq!(
            PortError::ParseFailed {
                cmd: "ps".into(),
                reason: "unexpected column count".into(),
            }
            .user_message(),
            "system process output could not be parsed"
        );
        assert_eq!(
            PortError::ExecFailed {
                cmd: "ps".into(),
                exit: 2,
                stderr: "bad flag".into(),
            }
            .user_message(),
            "system inspection failed"
        );
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
    fn verbose_log_port_error_keeps_full_details_not_user_summary() {
        let _guard = verbose_test_lock().lock().unwrap();
        set_verbose_enabled(true);
        drain_verbose_log();

        let error = PortError::ExecFailed {
            cmd: "ps".into(),
            exit: 2,
            stderr: "bad flag".into(),
        };
        let summary = error.user_message();
        let full = error.full();

        verbose_log_port_error(&error);

        let drained = drain_verbose_log();
        assert_eq!(drained, vec![full]);
        assert_ne!(drained[0], summary);
        assert!(drained[0].contains("bad flag"));

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

    #[test]
    fn user_warning_buffer_records_user_messages_and_drains() {
        let _guard = verbose_test_lock().lock().unwrap();
        drain_user_warnings();

        record_user_warning(&PortError::CommandMissing("lsof".into()));
        record_user_warning(&PortError::PermissionDenied("ps".into()));

        let drained = drain_user_warnings();
        assert_eq!(drained.len(), 2);
        assert_eq!(
            drained[0].user_message(),
            "lsof not found; results may be incomplete"
        );
        assert_eq!(
            drained[1].user_message(),
            "permission denied while inspecting processes"
        );
        assert!(drain_user_warnings().is_empty());
    }
}
