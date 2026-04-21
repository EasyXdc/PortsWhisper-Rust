use serde::Serialize;
use std::path::Path;

use crate::model::{PortInfo, ProcessInfo};

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct CommandEnvelope<T>
where
    T: Serialize,
{
    pub command: String,
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<ErrorPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

impl<T> CommandEnvelope<T>
where
    T: Serialize,
{
    pub fn ok(command: impl Into<String>, data: T) -> Self {
        Self {
            command: command.into(),
            ok: true,
            data: Some(data),
            error: None,
            warnings: None,
        }
    }

    pub fn err(
        command: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            command: command.into(),
            ok: false,
            data: None,
            error: Some(ErrorPayload {
                code: code.into(),
                message: message.into(),
            }),
            warnings: None,
        }
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        if !warnings.is_empty() {
            self.warnings = Some(warnings);
        }
        self
    }
}

pub fn render_json<T>(value: &CommandEnvelope<T>) -> serde_json::Result<String>
where
    T: Serialize,
{
    serde_json::to_string(value)
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct PortListPayload {
    pub ports: Vec<PortSummary>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ProcessListPayload {
    pub processes: Vec<ProcessSummary>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct PortDetailPayload {
    pub port: Option<PortDetail>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct KillPayload {
    pub signal: String,
    pub targets: Vec<KillTargetPayload>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct KillTargetPayload {
    pub input: String,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub via: Option<String>,
    pub process_name: Option<String>,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct CleanPayload {
    pub confirmed: bool,
    pub orphaned: Vec<PortSummary>,
    pub killed: Vec<u32>,
    pub failed: Vec<u32>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct LogsPayload {
    pub pid: u32,
    pub port: Option<u16>,
    pub process_name: String,
    pub follow: bool,
    pub lines: String,
    pub stderr_only: bool,
    pub source: Option<LogSourcePayload>,
    pub output: Option<String>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct HelpPayload {
    pub usage: Vec<String>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct CheckPayload {
    pub ports: Vec<CheckPortPayload>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct CheckPortPayload {
    pub port: u16,
    pub available: bool,
    pub occupied: bool,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct WatchLinePayload {
    pub r#type: String,
    pub action: Option<String>,
    pub message: Option<String>,
    pub port: Option<PortSummary>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct LogSourcePayload {
    pub kind: String,
    pub path: Option<String>,
    pub command: Option<String>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct PortSummary {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
    pub command: String,
    pub cwd: Option<String>,
    pub project_name: Option<String>,
    pub framework: Option<String>,
    pub uptime: Option<String>,
    pub status: String,
    pub memory: Option<String>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct PortDetail {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
    pub command: String,
    pub cwd: Option<String>,
    pub project_name: Option<String>,
    pub framework: Option<String>,
    pub uptime: Option<String>,
    pub start_time: Option<String>,
    pub status: String,
    pub memory: Option<String>,
    pub git_branch: Option<String>,
    pub process_tree: Vec<ProcessTreeNodePayload>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ProcessSummary {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub process_name: String,
    pub command: String,
    pub description: String,
    pub cpu: f32,
    pub memory: Option<String>,
    pub cwd: Option<String>,
    pub project_name: Option<String>,
    pub framework: Option<String>,
    pub uptime: Option<String>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct ProcessTreeNodePayload {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
}

pub fn list_payload(ports: &[PortInfo]) -> PortListPayload {
    PortListPayload {
        ports: ports.iter().map(PortSummary::from).collect(),
    }
}

pub fn process_list_payload(processes: &[ProcessInfo]) -> ProcessListPayload {
    ProcessListPayload {
        processes: processes.iter().map(ProcessSummary::from).collect(),
    }
}

pub fn detail_payload(port: Option<&PortInfo>) -> PortDetailPayload {
    PortDetailPayload {
        port: port.map(PortDetail::from),
    }
}

pub fn kill_payload(signal: impl Into<String>, targets: Vec<KillTargetPayload>) -> KillPayload {
    KillPayload {
        signal: signal.into(),
        targets,
    }
}

pub fn clean_payload(
    confirmed: bool,
    orphaned: &[PortInfo],
    killed: Vec<u32>,
    failed: Vec<u32>,
) -> CleanPayload {
    CleanPayload {
        confirmed,
        orphaned: orphaned.iter().map(PortSummary::from).collect(),
        killed,
        failed,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn logs_payload(
    pid: u32,
    port: Option<u16>,
    process_name: impl Into<String>,
    follow: bool,
    lines: impl Into<String>,
    stderr_only: bool,
    source: Option<LogSourcePayload>,
    output: Option<String>,
) -> LogsPayload {
    LogsPayload {
        pid,
        port,
        process_name: process_name.into(),
        follow,
        lines: lines.into(),
        stderr_only,
        source,
        output,
    }
}

pub fn help_payload(usage: &[String]) -> HelpPayload {
    HelpPayload {
        usage: usage.to_vec(),
    }
}

pub fn check_payload(results: &[crate::check::PortCheckResult]) -> CheckPayload {
    CheckPayload {
        ports: results.iter().map(CheckPortPayload::from).collect(),
    }
}

pub fn watch_warning_payload(message: impl Into<String>) -> WatchLinePayload {
    WatchLinePayload {
        r#type: "warning".to_string(),
        action: None,
        message: Some(message.into()),
        port: None,
    }
}

pub fn watch_event_payload(action: impl Into<String>, port: &PortInfo) -> WatchLinePayload {
    WatchLinePayload {
        r#type: "event".to_string(),
        action: Some(action.into()),
        message: None,
        port: Some(PortSummary::from(port)),
    }
}

impl From<&PortInfo> for PortSummary {
    fn from(port: &PortInfo) -> Self {
        Self {
            port: port.port,
            pid: port.pid,
            process_name: port.process_name.clone(),
            command: port.command.clone(),
            cwd: port.cwd.as_deref().map(path_to_string),
            project_name: port.project_name.clone(),
            framework: port.framework.clone(),
            uptime: port.uptime.clone(),
            status: port.status.label().to_string(),
            memory: port.memory.clone(),
        }
    }
}

impl From<&PortInfo> for PortDetail {
    fn from(port: &PortInfo) -> Self {
        Self {
            port: port.port,
            pid: port.pid,
            process_name: port.process_name.clone(),
            command: port.command.clone(),
            cwd: port.cwd.as_deref().map(path_to_string),
            project_name: port.project_name.clone(),
            framework: port.framework.clone(),
            uptime: port.uptime.clone(),
            start_time: port.start_time.as_ref().map(ToString::to_string),
            status: port.status.label().to_string(),
            memory: port.memory.clone(),
            git_branch: port.git_branch.clone(),
            process_tree: port
                .process_tree
                .iter()
                .map(ProcessTreeNodePayload::from)
                .collect(),
        }
    }
}

impl From<&ProcessInfo> for ProcessSummary {
    fn from(process: &ProcessInfo) -> Self {
        Self {
            pid: process.pid,
            ppid: process.ppid,
            process_name: process.process_name.clone(),
            command: process.command.clone(),
            description: process.description.clone(),
            cpu: process.cpu,
            memory: process.memory.clone(),
            cwd: process.cwd.as_deref().map(path_to_string),
            project_name: process.project_name.clone(),
            framework: process.framework.clone(),
            uptime: process.uptime.clone(),
        }
    }
}

impl From<&crate::model::ProcessTreeNode> for ProcessTreeNodePayload {
    fn from(node: &crate::model::ProcessTreeNode) -> Self {
        Self {
            pid: node.pid,
            ppid: node.ppid,
            name: node.name.clone(),
        }
    }
}

impl From<&crate::check::PortCheckResult> for CheckPortPayload {
    fn from(result: &crate::check::PortCheckResult) -> Self {
        Self {
            port: result.port,
            available: result.available,
            occupied: result.occupied,
        }
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        CommandEnvelope, KillTargetPayload, LogSourcePayload, check_payload, clean_payload,
        detail_payload, kill_payload, list_payload, logs_payload, process_list_payload,
        render_json,
    };
    use crate::model::{DisplayTime, PortInfo, ProcessInfo, ProcessStatus, ProcessTreeNode};
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn serializes_success_envelope_with_data_payload() {
        let rendered = render_json(&CommandEnvelope::ok("ps", vec!["3000"]))
            .expect("json render should succeed");

        assert_eq!(
            rendered,
            "{\"command\":\"ps\",\"ok\":true,\"data\":[\"3000\"],\"error\":null}"
        );
    }

    #[test]
    fn serializes_error_envelope_without_data_payload() {
        let rendered = render_json(&CommandEnvelope::<Vec<String>>::err(
            "ports logs 3000 -f",
            "unsupported_follow",
            "follow mode is not supported with --json yet",
        ))
        .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports logs 3000 -f",
                "ok": false,
                "data": null,
                "error": {
                    "code": "unsupported_follow",
                    "message": "follow mode is not supported with --json yet"
                }
            })
        );
    }

    #[test]
    fn serializes_list_payload_with_port_fields() {
        let rendered = render_json(&CommandEnvelope::ok(
            "ports",
            list_payload(&[port_fixture()]),
        ))
        .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports",
                "ok": true,
                "data": {
                    "ports": [
                        {
                            "port": 3000,
                            "pid": 42,
                            "process_name": "node",
                            "command": "node server.js",
                            "cwd": "/repo/demo",
                            "project_name": "demo",
                            "framework": "Vite",
                            "uptime": "1m 2s",
                            "status": "healthy",
                            "memory": "12.0 MB"
                        }
                    ]
                },
                "error": null
            })
        );
    }

    #[test]
    fn serializes_process_list_payload_with_process_fields() {
        let rendered = render_json(&CommandEnvelope::ok(
            "ports ps",
            process_list_payload(&[process_fixture()]),
        ))
        .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports ps",
                "ok": true,
                "data": {
                    "processes": [
                        {
                            "pid": 42,
                            "ppid": 1,
                            "process_name": "node",
                            "command": "node server.js",
                            "description": "dev server",
                            "cpu": 6.5,
                            "memory": "12.0 MB",
                            "cwd": "/repo/demo",
                            "project_name": "demo",
                            "framework": "Vite",
                            "uptime": "1m 2s"
                        }
                    ]
                },
                "error": null
            })
        );
    }

    #[test]
    fn serializes_detail_payload_with_nested_process_tree() {
        let rendered = render_json(&CommandEnvelope::ok(
            "ports 3000",
            detail_payload(Some(&detail_fixture())),
        ))
        .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports 3000",
                "ok": true,
                "data": {
                    "port": {
                        "port": 3000,
                        "pid": 42,
                        "process_name": "node",
                        "command": "node server.js",
                        "cwd": "/repo/demo",
                        "project_name": "demo",
                        "framework": "Vite",
                        "uptime": "1m 2s",
                        "start_time": "Fri Apr 17 10:00:00 2026",
                        "status": "orphaned",
                        "memory": "12.0 MB",
                        "git_branch": "main",
                        "process_tree": [
                            {
                                "pid": 42,
                                "ppid": 1,
                                "name": "node"
                            },
                            {
                                "pid": 1,
                                "ppid": null,
                                "name": "launchd"
                            }
                        ]
                    }
                },
                "error": null
            })
        );
    }

    #[test]
    fn serializes_detail_payload_with_null_port_when_missing() {
        let rendered = render_json(&CommandEnvelope::ok("ports 39999", detail_payload(None)))
            .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports 39999",
                "ok": true,
                "data": {
                    "port": null
                },
                "error": null
            })
        );
    }

    #[test]
    fn serializes_kill_payload_with_target_results() {
        let rendered = render_json(&CommandEnvelope::ok(
            "ports kill 3000 3001",
            kill_payload(
                "SIGTERM",
                vec![
                    KillTargetPayload {
                        input: "3000".to_string(),
                        pid: Some(42),
                        port: Some(3000),
                        via: Some("port".to_string()),
                        process_name: Some("node".to_string()),
                        success: true,
                        message: "Sent SIGTERM to :3000 — node (PID 42)".to_string(),
                    },
                    KillTargetPayload {
                        input: "3001".to_string(),
                        pid: None,
                        port: None,
                        via: None,
                        process_name: None,
                        success: false,
                        message: "No listener on :3001 and no process with PID 3001".to_string(),
                    },
                ],
            ),
        ))
        .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports kill 3000 3001",
                "ok": true,
                "data": {
                    "signal": "SIGTERM",
                    "targets": [
                        {
                            "input": "3000",
                            "pid": 42,
                            "port": 3000,
                            "via": "port",
                            "process_name": "node",
                            "success": true,
                            "message": "Sent SIGTERM to :3000 — node (PID 42)"
                        },
                        {
                            "input": "3001",
                            "pid": null,
                            "port": null,
                            "via": null,
                            "process_name": null,
                            "success": false,
                            "message": "No listener on :3001 and no process with PID 3001"
                        }
                    ]
                },
                "error": null
            })
        );
    }

    #[test]
    fn serializes_clean_payload_with_orphaned_process_results() {
        let rendered = render_json(&CommandEnvelope::ok(
            "ports clean",
            clean_payload(true, &[port_fixture()], vec![42], vec![7]),
        ))
        .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports clean",
                "ok": true,
                "data": {
                    "confirmed": true,
                    "orphaned": [
                        {
                            "port": 3000,
                            "pid": 42,
                            "process_name": "node",
                            "command": "node server.js",
                            "cwd": "/repo/demo",
                            "project_name": "demo",
                            "framework": "Vite",
                            "uptime": "1m 2s",
                            "status": "healthy",
                            "memory": "12.0 MB"
                        }
                    ],
                    "killed": [42],
                    "failed": [7]
                },
                "error": null
            })
        );
    }

    #[test]
    fn serializes_logs_payload_with_source_and_output() {
        let rendered = render_json(&CommandEnvelope::ok(
            "ports logs 3000 --lines=5",
            logs_payload(
                42,
                Some(3000),
                "node",
                false,
                "5",
                false,
                Some(LogSourcePayload {
                    kind: "file".to_string(),
                    path: Some("/tmp/app.log".to_string()),
                    command: None,
                }),
                Some("ready\nrequest /health".to_string()),
            ),
        ))
        .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports logs 3000 --lines=5",
                "ok": true,
                "data": {
                    "pid": 42,
                    "port": 3000,
                    "process_name": "node",
                    "follow": false,
                    "lines": "5",
                    "stderr_only": false,
                    "source": {
                        "kind": "file",
                        "path": "/tmp/app.log",
                        "command": null
                    },
                    "output": "ready\nrequest /health"
                },
                "error": null
            })
        );
    }

    #[test]
    fn serializes_check_payload_with_port_availability() {
        let rendered = render_json(&CommandEnvelope::ok(
            "ports check",
            check_payload(&[
                crate::check::PortCheckResult {
                    port: 3000,
                    available: false,
                    occupied: true,
                },
                crate::check::PortCheckResult {
                    port: 5173,
                    available: true,
                    occupied: false,
                },
            ]),
        ))
        .expect("json render should succeed");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
            json!({
                "command": "ports check",
                "ok": true,
                "data": {
                    "ports": [
                        {
                            "port": 3000,
                            "available": false,
                            "occupied": true
                        },
                        {
                            "port": 5173,
                            "available": true,
                            "occupied": false
                        }
                    ]
                },
                "error": null
            })
        );
    }

    fn port_fixture() -> PortInfo {
        PortInfo {
            port: 3000,
            pid: 42,
            process_name: "node".to_string(),
            raw_name: "node".to_string(),
            command: "node server.js".to_string(),
            cwd: Some(PathBuf::from("/repo/demo")),
            project_name: Some("demo".to_string()),
            framework: Some("Vite".to_string()),
            uptime: Some("1m 2s".to_string()),
            start_time: None,
            status: ProcessStatus::Healthy,
            memory: Some("12.0 MB".to_string()),
            git_branch: None,
            process_tree: Vec::new(),
        }
    }

    fn detail_fixture() -> PortInfo {
        PortInfo {
            start_time: Some("Fri Apr 17 10:00:00 2026".parse::<DisplayTime>().unwrap()),
            status: ProcessStatus::Orphaned,
            git_branch: Some("main".to_string()),
            process_tree: vec![
                ProcessTreeNode {
                    pid: 42,
                    ppid: Some(1),
                    name: "node".to_string(),
                },
                ProcessTreeNode {
                    pid: 1,
                    ppid: None,
                    name: "launchd".to_string(),
                },
            ],
            ..port_fixture()
        }
    }

    fn process_fixture() -> ProcessInfo {
        ProcessInfo {
            pid: 42,
            ppid: Some(1),
            process_name: "node".to_string(),
            command: "node server.js".to_string(),
            description: "dev server".to_string(),
            cpu: 6.5,
            rss_kb: 12_288,
            memory: Some("12.0 MB".to_string()),
            cwd: Some(PathBuf::from("/repo/demo")),
            project_name: Some("demo".to_string()),
            framework: Some("Vite".to_string()),
            uptime: Some("1m 2s".to_string()),
            status_raw: "S".to_string(),
        }
    }
}
