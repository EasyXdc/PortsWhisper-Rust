use crate::model::{PortInfo, ProcessInfo, ProcessStatus, ProcessTreeNode};
use crate::style;
use crate::util::{truncate_visible, visible_width};

const BOX_INNER_WIDTH: usize = 37;
const PORT_HEADERS: [&str; 7] = [
    "PORT",
    "PROCESS",
    "PID",
    "PROJECT",
    "FRAMEWORK",
    "UPTIME",
    "STATUS",
];
const PROCESS_HEADERS: [&str; 8] = [
    "PID",
    "PROCESS",
    "CPU%",
    "MEM",
    "PROJECT",
    "FRAMEWORK",
    "UPTIME",
    "WHAT",
];

pub fn render_header() {
    let border = "─".repeat(BOX_INNER_WIDTH);
    println!();
    println!("{}", style::cyan_bold(format!(" ┌{border}┐")));
    println!(
        "{}{}{}",
        style::cyan_bold(" │"),
        style::white_bold(pad_to_width("  🔊 Port Whisperer", BOX_INNER_WIDTH)),
        style::cyan_bold("│")
    );
    println!(
        "{}{}{}",
        style::cyan_bold(" │"),
        style::gray(pad_to_width(
            "  listening to your ports...",
            BOX_INNER_WIDTH
        )),
        style::cyan_bold("│")
    );
    println!("{}", style::cyan_bold(format!(" └{border}┘")));
    println!();
}

pub fn display_port_table(ports: &[PortInfo], filtered: bool) {
    render_header();
    if ports.is_empty() {
        print!("{}", render_empty_port_table_message());
        return;
    }

    let rows = port_table_rows(ports);
    print_table(&PORT_HEADERS, &rows);
    println!();
    println!("{}", port_summary_line(ports.len(), filtered));
    println!();
}

fn render_empty_port_table_message() -> String {
    format!(
        "{}\n{}{}{}\n",
        style::gray("  No active listening ports found.\n"),
        style::gray("  Start a dev server and run "),
        style::cyan("ports"),
        style::gray(" again.\n")
    )
}

fn port_table_rows(ports: &[PortInfo]) -> Vec<Vec<String>> {
    ports
        .iter()
        .map(|p| {
            vec![
                style::white_bold(format!(":{}", p.port)),
                style::white(if p.process_name.is_empty() {
                    p.raw_name.as_str()
                } else {
                    p.process_name.as_str()
                }),
                style::gray(p.pid.to_string()),
                p.project_name
                    .as_ref()
                    .map(|v| style::blue(truncate_visible(v, 20)))
                    .unwrap_or_else(|| style::gray("—")),
                p.framework
                    .as_ref()
                    .map(|v| style::framework(v))
                    .unwrap_or_else(|| style::gray("—")),
                p.uptime
                    .as_ref()
                    .map(style::yellow)
                    .unwrap_or_else(|| style::gray("—")),
                format_status(&p.status),
            ]
        })
        .collect()
}

fn port_summary_line(count: usize, filtered: bool) -> String {
    let all_hint = if filtered {
        format!(
            "{}{}{}",
            style::gray("  ·  "),
            style::cyan("--all"),
            style::gray(" to show everything")
        )
    } else {
        String::new()
    };
    format!(
        "{}{}{}{}",
        style::gray(format!(
            "  {count} port{} active  ·  ",
            if count == 1 { "" } else { "s" }
        )),
        style::gray("Run "),
        style::cyan("ports <number>"),
        style::gray(" for details") + &all_hint
    )
}

pub fn display_process_table(processes: &[ProcessInfo], filtered: bool) {
    render_header();
    if processes.is_empty() {
        print!("{}", render_empty_process_table_message());
        return;
    }
    let rows = process_table_rows(processes);
    print_table(&PROCESS_HEADERS, &rows);
    println!();
    let all_hint = if filtered {
        format!(
            "{}{}{}",
            style::gray("  ·  "),
            style::cyan("--all"),
            style::gray(" to show everything")
        )
    } else {
        String::new()
    };
    println!(
        "{}{}",
        style::gray(format!(
            "  {} process{}",
            processes.len(),
            if processes.len() == 1 { "" } else { "es" }
        )),
        all_hint
    );
    println!();
}

fn render_empty_process_table_message() -> String {
    format!(
        "{}\n{}{}{}\n",
        style::gray("  No dev processes found.\n"),
        style::gray("  Run "),
        style::cyan("ports ps --all"),
        style::gray(" to show all processes.\n")
    )
}

fn process_table_rows(processes: &[ProcessInfo]) -> Vec<Vec<String>> {
    processes
        .iter()
        .map(|p| {
            let cpu = format!("{:.1}", p.cpu);
            let cpu = if p.cpu > 25.0 {
                style::red(cpu)
            } else if p.cpu > 5.0 {
                style::yellow(cpu)
            } else {
                style::green(cpu)
            };
            vec![
                style::gray(p.pid.to_string()),
                style::white_bold(truncate_visible(&p.process_name, 15)),
                cpu,
                p.memory
                    .as_ref()
                    .map(style::green)
                    .unwrap_or_else(|| style::gray("—")),
                p.project_name
                    .as_ref()
                    .map(|v| style::blue(truncate_visible(v, 20)))
                    .unwrap_or_else(|| style::gray("—")),
                p.framework
                    .as_ref()
                    .map(|v| style::framework(v))
                    .unwrap_or_else(|| style::gray("—")),
                p.uptime
                    .as_ref()
                    .map(style::yellow)
                    .unwrap_or_else(|| style::gray("—")),
                style::gray(truncate_visible(&p.description, 30)),
            ]
        })
        .collect()
}

pub fn display_port_detail(info: Option<&PortInfo>) {
    render_header();
    print!("{}", render_port_detail_body(info));
}

fn render_port_detail_body(info: Option<&PortInfo>) -> String {
    let Some(info) = info else {
        return format!("{}\n", style::red("  No process found on that port.\n"));
    };
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        style::white_bold(format!("  Port :{}", info.port))
    ));
    out.push_str(&format!("{}\n", style::gray("  ──────────────────────")));
    out.push('\n');
    push_field(
        &mut out,
        "Process",
        &style::white_bold(if info.process_name.is_empty() {
            &info.raw_name
        } else {
            &info.process_name
        }),
    );
    push_field(&mut out, "PID", &style::gray(info.pid.to_string()));
    push_field(&mut out, "Status", &format_status(&info.status));
    push_field(
        &mut out,
        "Framework",
        &info
            .framework
            .as_ref()
            .map(|v| style::framework(v))
            .unwrap_or_else(|| style::gray("—")),
    );
    push_field(
        &mut out,
        "Memory",
        &info
            .memory
            .as_ref()
            .map(style::green)
            .unwrap_or_else(|| style::gray("—")),
    );
    push_field(
        &mut out,
        "Uptime",
        &info
            .uptime
            .as_ref()
            .map(style::yellow)
            .unwrap_or_else(|| style::gray("—")),
    );
    if let Some(started) = &info.start_time {
        push_field(&mut out, "Started", &style::gray(started.to_string()));
    }
    out.push('\n');
    out.push_str(&format!("{}\n", style::cyan_bold("  Location")));
    out.push_str(&format!("{}\n", style::gray("  ──────────────────────")));
    push_field(
        &mut out,
        "Directory",
        &info
            .cwd
            .as_ref()
            .map(|p| style::blue(p.to_string_lossy()))
            .unwrap_or_else(|| style::gray("—")),
    );
    push_field(
        &mut out,
        "Project",
        &info
            .project_name
            .as_ref()
            .map(style::white)
            .unwrap_or_else(|| style::gray("—")),
    );
    push_field(
        &mut out,
        "Git Branch",
        &info
            .git_branch
            .as_ref()
            .map(style::magenta)
            .unwrap_or_else(|| style::gray("—")),
    );
    if !info.process_tree.is_empty() {
        out.push('\n');
        out.push_str(&format!("{}\n", style::cyan_bold("  Process Tree")));
        out.push_str(&format!("{}\n", style::gray("  ──────────────────────")));
        for (idx, node) in info.process_tree.iter().enumerate() {
            out.push_str(&render_process_tree_line(idx, node, info.pid));
        }
    }
    out.push('\n');
    out.push_str(&format!(
        "{}{}{}{}{}",
        style::gray("  Kill: "),
        style::cyan(format!("ports kill {}", info.port)),
        style::gray(" or "),
        style::cyan(format!("ports kill -f {}", info.port)),
        style::gray(" (force)")
    ));
    out.push_str("\n\n");
    out
}

pub fn display_clean_results(orphaned: &[PortInfo], killed: &[u32], failed: &[u32]) {
    render_header();
    print!("{}", render_clean_results_body(orphaned, killed, failed));
}

fn render_clean_results_body(orphaned: &[PortInfo], killed: &[u32], failed: &[u32]) -> String {
    if orphaned.is_empty() {
        return format!(
            "{}\n",
            style::green("  ✓ No orphaned or zombie processes found. All clean!\n")
        );
    }
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        style::yellow_bold(format!(
            "  Found {} orphaned/zombie process{}:\n",
            orphaned.len(),
            if orphaned.len() == 1 { "" } else { "es" }
        ))
    ));
    for p in orphaned {
        let icon = if killed.contains(&p.pid) {
            style::green("✓")
        } else if failed.contains(&p.pid) {
            style::red("✕")
        } else {
            style::yellow("?")
        };
        out.push_str(&format!(
            "  {} :{} {} {} {}",
            icon,
            style::white_bold(p.port.to_string()),
            style::gray("—"),
            p.process_name,
            style::gray(format!("(PID {})", p.pid))
        ));
        out.push('\n');
        if failed.contains(&p.pid) {
            out.push_str(&format!(
                "{}\n",
                style::red(format!("    Failed to kill. Try: sudo kill -9 {}", p.pid))
            ));
        }
    }
    out.push('\n');
    if !killed.is_empty() {
        out.push_str(&format!(
            "{}\n",
            style::green(format!(
                "  Cleaned {} process{}.",
                killed.len(),
                if killed.len() == 1 { "" } else { "es" }
            ))
        ));
    }
    if !failed.is_empty() {
        out.push_str(&format!(
            "{}\n",
            style::red(format!(
                "  Failed to clean {} process{}.",
                failed.len(),
                if failed.len() == 1 { "" } else { "es" }
            ))
        ));
    }
    out.push('\n');
    out
}

pub fn display_watch_header() {
    render_header();
    println!("{}", style::cyan_bold("  Watching for port changes..."));
    println!("{}", style::gray("  Press Ctrl+C to stop\n"));
}

pub fn display_watch_event(kind: &str, info: &PortInfo) {
    let timestamp = style::gray(current_time_label());
    println!("{}", render_watch_event_line(kind, info, &timestamp));
}

fn render_watch_event_line(kind: &str, info: &PortInfo, timestamp: &str) -> String {
    if kind == "new" {
        let project = info
            .project_name
            .as_ref()
            .map(|p| style::blue(format!(" [{p}]")))
            .unwrap_or_default();
        let framework = info
            .framework
            .as_ref()
            .map(|f| format!(" {}", style::framework(f)))
            .unwrap_or_default();
        format!(
            "  {} {}    :{} ← {}{}{}",
            timestamp,
            style::green("▲ NEW"),
            style::white_bold(info.port.to_string()),
            style::white(&info.process_name),
            project,
            framework
        )
    } else {
        format!(
            "  {} {} :{}",
            timestamp,
            style::red("▼ CLOSED"),
            style::white_bold(info.port.to_string())
        )
    }
}

fn push_field(out: &mut String, label: &str, value: &str) {
    out.push_str(&field_line(label, value));
}

fn field_line(label: &str, value: &str) -> String {
    format!("  {} {}\n", style::gray(format!("{label:<16}")), value)
}

fn render_process_tree_line(idx: usize, node: &ProcessTreeNode, current_pid: u32) -> String {
    let indent = "  ".repeat(idx);
    let prefix = if idx == 0 { "→" } else { "└─" };
    let name = if node.pid == current_pid {
        style::white_bold(&node.name)
    } else {
        style::gray(&node.name)
    };
    format!(
        "  {}{} {} {}\n",
        indent,
        style::gray(prefix),
        name,
        style::gray(format!("({})", node.pid))
    )
}

fn format_status(status: &ProcessStatus) -> String {
    match status {
        ProcessStatus::Healthy => format!("{} {}", style::green("●"), style::green(status.label())),
        ProcessStatus::Orphaned => {
            format!("{} {}", style::yellow("●"), style::yellow(status.label()))
        }
        ProcessStatus::Zombie => format!("{} {}", style::red("●"), style::red(status.label())),
        ProcessStatus::Unknown => format!("{} {}", style::gray("●"), style::gray(status.label())),
    }
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    print!("{}", render_table(headers, rows));
}

fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(visible_len(cell));
        }
    }
    let mut out = String::new();
    push_border(&mut out, "┌", "┬", "┐", &widths);
    out.push('│');
    for (idx, header) in headers.iter().enumerate() {
        out.push_str(&format!(
            " {}{} │",
            style::cyan_bold(header),
            " ".repeat(widths[idx] - header.len())
        ));
    }
    out.push('\n');
    push_border(&mut out, "├", "┼", "┤", &widths);
    for row in rows {
        out.push('│');
        for (idx, cell) in row.iter().enumerate() {
            out.push_str(&format!(
                " {}{} │",
                cell,
                " ".repeat(widths[idx] - visible_len(cell))
            ));
        }
        out.push('\n');
    }
    push_border(&mut out, "└", "┴", "┘", &widths);
    out
}

fn push_border(out: &mut String, left: &str, mid: &str, right: &str, widths: &[usize]) {
    out.push_str(left);
    for (idx, width) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(width + 2));
        if idx + 1 == widths.len() {
            out.push_str(right);
        } else {
            out.push_str(mid);
        }
    }
    out.push('\n');
}

fn visible_len(s: &str) -> usize {
    let mut visible = String::new();
    let mut esc = false;
    for ch in s.chars() {
        if esc {
            if ch == 'm' {
                esc = false;
            }
            continue;
        }
        if ch == '\x1b' {
            esc = true;
            continue;
        }
        visible.push(ch);
    }
    visible_width(&visible)
}

fn pad_to_width(s: &str, width: usize) -> String {
    let len = visible_width(s);
    format!("{}{}", s, " ".repeat(width.saturating_sub(len)))
}

fn current_time_label() -> String {
    std::process::Command::new("date")
        .arg("+%T")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "--:--:--".to_string())
}

#[cfg(test)]
fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut esc = false;
    for ch in s.chars() {
        if esc {
            if ch == 'm' {
                esc = false;
            }
            continue;
        }
        if ch == '\x1b' {
            esc = true;
            continue;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        PORT_HEADERS, PROCESS_HEADERS, port_summary_line, port_table_rows, process_table_rows,
        render_clean_results_body, render_empty_port_table_message,
        render_empty_process_table_message, render_port_detail_body, render_table,
        render_watch_event_line, strip_ansi, visible_len,
    };
    use crate::model::{DisplayTime, PortInfo, ProcessInfo, ProcessStatus, ProcessTreeNode};
    use std::path::PathBuf;

    #[test]
    fn port_table_fixture_contains_headers_missing_marker_and_summary() {
        let rows = port_table_rows(&[port_fixture()]);
        let table = strip_ansi(&render_table(&PORT_HEADERS, &rows));
        assert!(table.contains("PORT"));
        assert!(table.contains("PROCESS"));
        assert!(table.contains("PID"));
        assert!(table.contains("PROJECT"));
        assert!(table.contains("FRAMEWORK"));
        assert!(table.contains("UPTIME"));
        assert!(table.contains("STATUS"));
        assert!(table.contains(":3000"));
        assert!(table.contains("node"));
        assert!(table.contains("—"));

        let summary = strip_ansi(&port_summary_line(1, true));
        assert!(summary.contains("1 port active"));
        assert!(summary.contains("Run ports <number> for details"));
        assert!(summary.contains("--all to show everything"));
    }

    #[test]
    fn table_rendering_keeps_ansi_and_plain_text_visible() {
        let rows = port_table_rows(&[port_fixture()]);
        let table = render_table(&PORT_HEADERS, &rows);
        assert!(table.contains("\x1b["));
        assert!(strip_ansi(&table).contains("healthy"));
    }

    #[test]
    fn process_table_fixture_contains_expected_columns_and_values() {
        let rows = process_table_rows(&[process_fixture()]);
        let table = strip_ansi(&render_table(&PROCESS_HEADERS, &rows));
        assert!(table.contains("PID"));
        assert!(table.contains("PROCESS"));
        assert!(table.contains("CPU%"));
        assert!(table.contains("MEM"));
        assert!(table.contains("PROJECT"));
        assert!(table.contains("FRAMEWORK"));
        assert!(table.contains("UPTIME"));
        assert!(table.contains("WHAT"));
        assert!(table.contains("42"));
        assert!(table.contains("node"));
        assert!(table.contains("6.5"));
        assert!(table.contains("12.0 MB"));
        assert!(table.contains("demo"));
        assert!(table.contains("Vite"));
        assert!(table.contains("dev server"));
    }

    #[test]
    fn empty_table_fixtures_render_expected_guidance() {
        let port_empty = strip_ansi(&render_empty_port_table_message());
        assert!(port_empty.contains("No active listening ports found."));
        assert!(port_empty.contains("Start a dev server and run ports again."));

        let process_empty = strip_ansi(&render_empty_process_table_message());
        assert!(process_empty.contains("No dev processes found."));
        assert!(process_empty.contains("Run ports ps --all to show all processes."));
    }

    #[test]
    fn port_detail_fixture_contains_sections_fields_and_kill_hint() {
        let detail = strip_ansi(&render_port_detail_body(Some(&detailed_port_fixture())));
        assert!(detail.contains("Port :3000"));
        assert!(detail.contains("Process"));
        assert!(detail.contains("PID"));
        assert!(detail.contains("Status"));
        assert!(detail.contains("Framework"));
        assert!(detail.contains("Memory"));
        assert!(detail.contains("Uptime"));
        assert!(detail.contains("Started"));
        assert!(detail.contains("Location"));
        assert!(detail.contains("Directory"));
        assert!(detail.contains("Project"));
        assert!(detail.contains("Git Branch"));
        assert!(detail.contains("Process Tree"));
        assert!(detail.contains("ports kill 3000"));
        assert!(detail.contains("ports kill -f 3000"));

        let missing = strip_ansi(&render_port_detail_body(None));
        assert!(missing.contains("No process found on that port."));
    }

    #[test]
    fn clean_result_fixture_contains_status_rows_and_summary() {
        let orphaned = vec![detailed_port_fixture()];
        let clean = strip_ansi(&render_clean_results_body(&orphaned, &[42], &[]));
        assert!(clean.contains("Found 1 orphaned/zombie process"));
        assert!(clean.contains(":3000"));
        assert!(clean.contains("node"));
        assert!(clean.contains("PID 42"));
        assert!(clean.contains("Cleaned 1 process."));

        let failed = strip_ansi(&render_clean_results_body(&orphaned, &[], &[42]));
        assert!(failed.contains("Failed to kill. Try: sudo kill -9 42"));
        assert!(failed.contains("Failed to clean 1 process."));

        let empty = strip_ansi(&render_clean_results_body(&[], &[], &[]));
        assert!(empty.contains("No orphaned or zombie processes found. All clean!"));
    }

    #[test]
    fn watch_event_fixtures_render_new_and_removed_events() {
        let info = detailed_port_fixture();
        let new_event = strip_ansi(&render_watch_event_line("new", &info, "12:00:00"));
        assert!(new_event.contains("12:00:00"));
        assert!(new_event.contains("NEW"));
        assert!(new_event.contains(":3000"));
        assert!(new_event.contains("node"));
        assert!(new_event.contains("[demo]"));
        assert!(new_event.contains("Vite"));

        let closed_event = strip_ansi(&render_watch_event_line("closed", &info, "12:00:01"));
        assert!(closed_event.contains("12:00:01"));
        assert!(closed_event.contains("CLOSED"));
        assert!(closed_event.contains(":3000"));
    }

    #[test]
    fn table_headers_match_node_reference_columns() {
        assert_eq!(
            PORT_HEADERS,
            [
                "PORT",
                "PROCESS",
                "PID",
                "PROJECT",
                "FRAMEWORK",
                "UPTIME",
                "STATUS"
            ]
        );
        assert_eq!(
            PROCESS_HEADERS,
            [
                "PID",
                "PROCESS",
                "CPU%",
                "MEM",
                "PROJECT",
                "FRAMEWORK",
                "UPTIME",
                "WHAT",
            ]
        );
    }

    #[test]
    fn visible_width_truncation_handles_wide_unicode() {
        let rows = vec![vec!["表表表A".to_string()]];
        let table = strip_ansi(&render_table(&["PROJECT"], &rows));
        let line = table
            .lines()
            .find(|line| line.contains("表表表A"))
            .expect("data row should exist");
        assert!(line.ends_with(" │"), "row should stay box-aligned: {line}");
        assert_eq!(visible_len("表表表A"), 7);
    }

    fn port_fixture() -> PortInfo {
        PortInfo {
            port: 3000,
            pid: 42,
            process_name: "node".to_string(),
            raw_name: "node".to_string(),
            command: "node server.js".to_string(),
            cwd: None,
            project_name: None,
            framework: None,
            uptime: None,
            start_time: None,
            status: ProcessStatus::Healthy,
            memory: None,
            git_branch: None,
            process_tree: Vec::new(),
        }
    }

    fn detailed_port_fixture() -> PortInfo {
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
            start_time: Some("Fri Apr 17 10:00:00 2026".parse::<DisplayTime>().unwrap()),
            status: ProcessStatus::Orphaned,
            memory: Some("12.0 MB".to_string()),
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
            cwd: None,
            project_name: Some("demo".to_string()),
            framework: Some("Vite".to_string()),
            uptime: Some("1m 2s".to_string()),
            status_raw: "S".to_string(),
        }
    }
}
