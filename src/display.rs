use crate::model::{PortInfo, ProcessInfo, ProcessStatus, ProcessTreeNode};
use crate::style;
use crate::util::{truncate_visible, visible_width};
use std::io::IsTerminal;
use std::time::Duration;
use terminal_size::{Width, terminal_size};

const DEFAULT_HEADER_WIDTH: usize = 37;
const MIN_HEADER_WIDTH: usize = 20;
const MAX_HEADER_WIDTH: usize = 80;
const SLOW_COMMAND_SPINNER_THRESHOLD_MS: u64 = 120;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayConfig {
    pub decorative_header: bool,
    pub ascii: bool,
    pub spinner_enabled: bool,
    pub spinner_threshold: Duration,
    pub command_elapsed: Option<Duration>,
    pub terminal_width: Option<usize>,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        display_config_with(false, false, false, stdout_is_terminal, terminal_width)
    }
}

pub fn render_header_with_config(config: &DisplayConfig) -> String {
    if !config.decorative_header {
        return String::new();
    }

    let glyphs = style::glyphs_for(config.ascii);
    let header_width = resolve_header_width(config.terminal_width, terminal_width);
    let border = glyphs.horizontal.repeat(header_width);
    let spinner_line = render_slow_command_status_line(config, header_width);
    format!(
        "\n{}\n{}{}{}\n{}{}{}{}\n{}\n\n",
        style::cyan_bold(format!(
            " {}{border}{}",
            glyphs.header_left_top, glyphs.header_right_top
        )),
        style::cyan_bold(format!(" {}", glyphs.vertical)),
        style::white_bold(pad_to_width(
            &format!("  {} Port Whisperer", glyphs.speaker),
            header_width
        )),
        style::cyan_bold(glyphs.vertical),
        style::cyan_bold(format!(" {}", glyphs.vertical)),
        style::gray(pad_to_width("  listening to your ports...", header_width)),
        style::cyan_bold(glyphs.vertical),
        spinner_line,
        style::cyan_bold(format!(
            " {}{border}{}",
            glyphs.header_left_bottom, glyphs.header_right_bottom
        ))
    )
}

pub fn display_config(quiet: bool, ascii: bool, json: bool) -> DisplayConfig {
    display_config_with(quiet, ascii, json, stdout_is_terminal, terminal_width)
}

fn display_config_with(
    quiet: bool,
    ascii: bool,
    json: bool,
    is_terminal: impl FnOnce() -> bool,
    terminal_width_detector: impl FnOnce() -> Option<usize>,
) -> DisplayConfig {
    let interactive = is_terminal();
    let decorative_header = !quiet && interactive;
    DisplayConfig {
        decorative_header,
        ascii,
        spinner_enabled: !quiet && !json && interactive,
        spinner_threshold: slow_command_spinner_threshold(),
        command_elapsed: None,
        terminal_width: terminal_width_detector(),
    }
}

pub fn slow_command_spinner_threshold() -> Duration {
    Duration::from_millis(SLOW_COMMAND_SPINNER_THRESHOLD_MS)
}

pub fn should_show_slow_command_spinner(config: &DisplayConfig, elapsed: Duration) -> bool {
    config.spinner_enabled && elapsed >= config.spinner_threshold
}

pub fn with_command_elapsed(config: &DisplayConfig, elapsed: Duration) -> DisplayConfig {
    let mut next = *config;
    next.command_elapsed = Some(elapsed);
    next
}

fn render_slow_command_status_line(config: &DisplayConfig, header_width: usize) -> String {
    let Some(elapsed) = config.command_elapsed else {
        return String::new();
    };
    if !should_show_slow_command_spinner(config, elapsed) {
        return String::new();
    }

    format!(
        "\n{}{}{}",
        style::cyan_bold(" │"),
        style::yellow(pad_to_width("  Working...", header_width)),
        style::cyan_bold("│")
    )
}

fn stdout_is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

pub fn display_port_table_with_config(ports: &[PortInfo], filtered: bool, config: &DisplayConfig) {
    print!("{}", render_port_table_output(ports, filtered, config));
}

fn render_port_table_output(ports: &[PortInfo], filtered: bool, config: &DisplayConfig) -> String {
    let mut out = render_header_with_config(config);
    if ports.is_empty() {
        out.push_str(&render_empty_port_table_message());
        return out;
    }

    let rows = port_table_rows(ports, config);
    out.push_str(&render_table(&PORT_HEADERS, &rows, config));

    if config.decorative_header {
        out.push('\n');
        out.push_str(&port_summary_line(ports.len(), filtered));
        out.push_str("\n\n");
    }

    out
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

fn port_table_rows(ports: &[PortInfo], config: &DisplayConfig) -> Vec<Vec<String>> {
    if !config.decorative_header {
        return port_table_rows_plain(ports);
    }

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

fn port_table_rows_plain(ports: &[PortInfo]) -> Vec<Vec<String>> {
    ports
        .iter()
        .map(|p| {
            vec![
                format!(":{}", p.port),
                if p.process_name.is_empty() {
                    p.raw_name.clone()
                } else {
                    p.process_name.clone()
                },
                p.pid.to_string(),
                p.project_name.clone().unwrap_or_else(|| "-".to_string()),
                p.framework.clone().unwrap_or_else(|| "-".to_string()),
                p.uptime.clone().unwrap_or_else(|| "-".to_string()),
                p.status.label().to_string(),
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

pub fn display_process_table_with_config(
    processes: &[ProcessInfo],
    filtered: bool,
    config: &DisplayConfig,
) {
    print!(
        "{}",
        render_process_table_output(processes, filtered, config)
    );
}

fn render_process_table_output(
    processes: &[ProcessInfo],
    filtered: bool,
    config: &DisplayConfig,
) -> String {
    let mut out = render_header_with_config(config);
    if processes.is_empty() {
        out.push_str(&render_empty_process_table_message());
        return out;
    }

    let rows = process_table_rows(processes, config);
    out.push_str(&render_table(&PROCESS_HEADERS, &rows, config));

    if config.decorative_header {
        out.push('\n');
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
        out.push_str(&format!(
            "{}{}",
            style::gray(format!(
                "  {} process{}",
                processes.len(),
                if processes.len() == 1 { "" } else { "es" }
            )),
            all_hint
        ));
        out.push_str("\n\n");
    }

    out
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

fn process_table_rows(processes: &[ProcessInfo], config: &DisplayConfig) -> Vec<Vec<String>> {
    if !config.decorative_header {
        return process_table_rows_plain(processes);
    }

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

fn process_table_rows_plain(processes: &[ProcessInfo]) -> Vec<Vec<String>> {
    processes
        .iter()
        .map(|p| {
            vec![
                p.pid.to_string(),
                truncate_visible(&p.process_name, 15),
                format!("{:.1}", p.cpu),
                p.memory.clone().unwrap_or_else(|| "-".to_string()),
                p.project_name.clone().unwrap_or_else(|| "-".to_string()),
                p.framework.clone().unwrap_or_else(|| "-".to_string()),
                p.uptime.clone().unwrap_or_else(|| "-".to_string()),
                truncate_visible(&p.description, 30),
            ]
        })
        .collect()
}

pub fn display_port_detail_with_config(info: Option<&PortInfo>, config: &DisplayConfig) {
    print!("{}", render_header_with_config(config));
    print!("{}", render_port_detail_body(info));
}

fn render_port_detail_body(info: Option<&PortInfo>) -> String {
    let Some(info) = info else {
        return format!("{}\n", style::red("  No process found on that port.\n"));
    };
    let mut out = String::new();
    let glyphs = style::glyphs();
    out.push_str(&format!(
        "{}\n",
        style::white_bold(format!("  Port :{}", info.port))
    ));
    out.push_str(&format!(
        "{}\n",
        style::gray(format!("  {}", glyphs.horizontal.repeat(22)))
    ));
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
    out.push_str(&format!(
        "{}\n",
        style::gray(format!("  {}", glyphs.horizontal.repeat(22)))
    ));
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
        out.push_str(&format!(
            "{}\n",
            style::gray(format!("  {}", glyphs.horizontal.repeat(22)))
        ));
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

pub fn display_clean_results_with_config(
    orphaned: &[PortInfo],
    killed: &[u32],
    failed: &[u32],
    config: &DisplayConfig,
) {
    print!("{}", render_header_with_config(config));
    print!("{}", render_clean_results_body(orphaned, killed, failed));
}

fn render_clean_results_body(orphaned: &[PortInfo], killed: &[u32], failed: &[u32]) -> String {
    let glyphs = style::glyphs();
    if orphaned.is_empty() {
        return format!(
            "{}\n",
            style::green(format!(
                "  {} No orphaned or zombie processes found. All clean!\n",
                glyphs.success
            ))
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
            style::green(glyphs.success)
        } else if failed.contains(&p.pid) {
            style::red(glyphs.failure)
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

pub fn display_watch_header_with_config(config: &DisplayConfig) {
    print!("{}", render_header_with_config(config));
    println!("{}", style::cyan_bold("  Watching for port changes..."));
    println!("{}", style::gray("  Press Ctrl+C to stop\n"));
}

pub fn display_watch_event(kind: &str, info: &PortInfo) {
    let timestamp = style::gray(current_time_label());
    println!("{}", render_watch_event_line(kind, info, &timestamp));
}

fn render_watch_event_line(kind: &str, info: &PortInfo, timestamp: &str) -> String {
    let glyphs = style::glyphs();
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
            "  {} {}    :{} {} {}{}{}",
            timestamp,
            style::green(glyphs.new_label),
            style::white_bold(info.port.to_string()),
            style::gray(glyphs.watch_arrow),
            style::white(&info.process_name),
            project,
            framework
        )
    } else {
        format!(
            "  {} {} :{}",
            timestamp,
            style::red(glyphs.closed_label),
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
    let glyphs = style::glyphs();
    let prefix = if idx == 0 {
        glyphs.tree_root
    } else {
        glyphs.tree_branch
    };
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
    let glyphs = style::glyphs();
    match status {
        ProcessStatus::Healthy => format!(
            "{} {}",
            style::green(glyphs.healthy),
            style::green(status.label())
        ),
        ProcessStatus::Orphaned => {
            format!(
                "{} {}",
                style::yellow(glyphs.orphaned),
                style::yellow(status.label())
            )
        }
        ProcessStatus::Zombie => format!(
            "{} {}",
            style::red(glyphs.zombie),
            style::red(status.label())
        ),
        ProcessStatus::Unknown => format!(
            "{} {}",
            style::gray(glyphs.unknown),
            style::gray(status.label())
        ),
    }
}

fn render_table(headers: &[&str], rows: &[Vec<String>], config: &DisplayConfig) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(visible_len(cell));
        }
    }

    if !config.decorative_header {
        return render_plain_table(headers, rows, &widths);
    }

    let glyphs = style::glyphs();
    let mut out = String::new();
    push_border(
        &mut out,
        glyphs.table_top_left,
        glyphs.table_top_mid,
        glyphs.table_top_right,
        &widths,
        glyphs.horizontal,
    );
    out.push_str(glyphs.vertical);
    for (idx, header) in headers.iter().enumerate() {
        out.push_str(&format!(
            " {}{} {}",
            style::cyan_bold(header),
            " ".repeat(widths[idx] - header.len()),
            glyphs.vertical
        ));
    }
    out.push('\n');
    push_border(
        &mut out,
        glyphs.table_mid_left,
        glyphs.table_mid_mid,
        glyphs.table_mid_right,
        &widths,
        glyphs.horizontal,
    );
    for row in rows {
        out.push_str(glyphs.vertical);
        for (idx, cell) in row.iter().enumerate() {
            out.push_str(&format!(
                " {}{} {}",
                cell,
                " ".repeat(widths[idx] - visible_len(cell)),
                glyphs.vertical
            ));
        }
        out.push('\n');
    }
    push_border(
        &mut out,
        glyphs.table_bottom_left,
        glyphs.table_bottom_mid,
        glyphs.table_bottom_right,
        &widths,
        glyphs.horizontal,
    );
    out
}

fn render_plain_table(headers: &[&str], rows: &[Vec<String>], widths: &[usize]) -> String {
    let mut out = String::new();
    push_plain_row(&mut out, headers.iter().copied(), widths);
    for row in rows {
        push_plain_row(&mut out, row.iter().map(String::as_str), widths);
    }
    out
}

fn push_plain_row<'a>(out: &mut String, cells: impl Iterator<Item = &'a str>, widths: &[usize]) {
    for (idx, cell) in cells.enumerate() {
        if idx > 0 {
            out.push_str("  ");
        }

        out.push_str(cell);
        out.push_str(&" ".repeat(widths[idx].saturating_sub(visible_len(cell))));
    }
    out.push('\n');
}

fn push_border(
    out: &mut String,
    left: &str,
    mid: &str,
    right: &str,
    widths: &[usize],
    horizontal: &str,
) {
    out.push_str(left);
    for (idx, width) in widths.iter().enumerate() {
        out.push_str(&horizontal.repeat(width + 2));
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

fn resolve_header_width<F>(configured_width: Option<usize>, detect_width: F) -> usize
where
    F: FnOnce() -> Option<usize>,
{
    configured_width
        .or_else(detect_width)
        .map(clamp_header_width)
        .unwrap_or(DEFAULT_HEADER_WIDTH)
}

fn clamp_header_width(width: usize) -> usize {
    width.clamp(MIN_HEADER_WIDTH, MAX_HEADER_WIDTH)
}

fn terminal_width() -> Option<usize> {
    detected_terminal_width(terminal_width_from_probe, terminal_width_from_env)
}

fn detected_terminal_width<Probe, Env>(probe_width: Probe, env_width: Env) -> Option<usize>
where
    Probe: FnOnce() -> Option<usize>,
    Env: FnOnce() -> Option<usize>,
{
    probe_width().or_else(env_width)
}

fn terminal_width_from_probe() -> Option<usize> {
    terminal_size().map(|(Width(width), _)| usize::from(width))
}

fn terminal_width_from_env() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
}

fn current_time_label() -> String {
    let duration = std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let hours = (total_secs / 3600) % 24;
    let minutes = (total_secs / 60) % 60;
    let seconds = total_secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
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
        DisplayConfig, PORT_HEADERS, PROCESS_HEADERS, clamp_header_width, detected_terminal_width,
        display_config_with, format_status, port_summary_line, port_table_rows, process_table_rows,
        render_clean_results_body, render_empty_port_table_message,
        render_empty_process_table_message, render_header_with_config, render_port_detail_body,
        render_port_table_output, render_process_table_output, render_process_tree_line,
        render_table, render_watch_event_line, resolve_header_width,
        should_show_slow_command_spinner, slow_command_spinner_threshold, strip_ansi, visible_len,
    };
    use crate::model::{DisplayTime, PortInfo, ProcessInfo, ProcessStatus, ProcessTreeNode};
    use std::path::PathBuf;
    use std::time::Duration;

    fn decorative_table_config() -> DisplayConfig {
        DisplayConfig {
            decorative_header: true,
            ascii: false,
            spinner_enabled: false,
            spinner_threshold: slow_command_spinner_threshold(),
            command_elapsed: None,
            terminal_width: None,
        }
    }

    fn quiet_table_config() -> DisplayConfig {
        DisplayConfig {
            decorative_header: false,
            ascii: false,
            spinner_enabled: false,
            spinner_threshold: slow_command_spinner_threshold(),
            command_elapsed: None,
            terminal_width: None,
        }
    }

    #[test]
    fn port_table_fixture_contains_headers_missing_marker_and_summary() {
        let rows = port_table_rows(&[port_fixture()], &decorative_table_config());
        let table = strip_ansi(&render_table(
            &PORT_HEADERS,
            &rows,
            &decorative_table_config(),
        ));
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
        let rows = port_table_rows(&[port_fixture()], &decorative_table_config());
        let table = render_table(&PORT_HEADERS, &rows, &decorative_table_config());
        assert!(table.contains("\x1b["));
        assert!(strip_ansi(&table).contains("healthy"));
    }

    #[test]
    fn quiet_port_table_rendering_omits_borders() {
        let quiet_config = quiet_table_config();
        let rows = port_table_rows(&[port_fixture()], &quiet_config);

        let table = render_table(&PORT_HEADERS, &rows, &quiet_config);

        assert!(table.contains("PORT"));
        assert!(table.contains(":3000"));
        assert!(
            !table.contains("\x1b["),
            "quiet table should be plain text: {table:?}"
        );
        assert!(table.contains("healthy"));
        assert!(!table.contains("● healthy"));
        assert!(!table.contains('┌'));
        assert!(!table.contains('├'));
        assert!(!table.contains('└'));
        assert!(!table.contains('│'));
    }

    #[test]
    fn quiet_table_output_omits_summary_lines() {
        let quiet_config = quiet_table_config();

        let port_output = render_port_table_output(&[port_fixture()], true, &quiet_config);
        assert!(!port_output.contains("Run ports <number> for details"));
        assert!(!port_output.contains("--all to show everything"));
        assert!(!port_output.contains("1 port active"));

        let process_output = render_process_table_output(&[process_fixture()], true, &quiet_config);
        assert!(!process_output.contains("1 process"));
        assert!(!process_output.contains("--all to show everything"));
    }

    #[test]
    fn process_table_fixture_contains_expected_columns_and_values() {
        let rows = process_table_rows(&[process_fixture()], &decorative_table_config());
        let table = strip_ansi(&render_table(
            &PROCESS_HEADERS,
            &rows,
            &decorative_table_config(),
        ));
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
    fn clean_results_use_ascii_safe_markers_when_requested() {
        let _guard = crate::style::glyph_test_lock().lock().unwrap();
        crate::style::set_force_ascii(true);
        let orphaned = vec![detailed_port_fixture()];
        let clean = strip_ansi(&render_clean_results_body(&orphaned, &[42], &[]));
        crate::style::set_force_ascii(false);

        assert!(
            clean.contains("v :3000"),
            "expected ascii-safe clean marker: {clean}"
        );
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
        let table = strip_ansi(&render_table(
            &["PROJECT"],
            &rows,
            &decorative_table_config(),
        ));
        let line = table
            .lines()
            .find(|line| line.contains("表表表A"))
            .expect("data row should exist");
        assert!(line.ends_with(" │"), "row should stay box-aligned: {line}");
        assert_eq!(visible_len("表表表A"), 7);
    }

    #[test]
    fn header_rendering_includes_decorative_banner_when_enabled() {
        let rendered = strip_ansi(&render_header_with_config(&DisplayConfig::default()));

        assert!(rendered.contains("Port Whisperer"));
        assert!(rendered.contains("listening to your ports..."));
        assert!(rendered.contains("┌"));
        assert!(rendered.contains("└"));
    }

    #[test]
    fn header_rendering_suppresses_decorative_banner_when_disabled() {
        let rendered = render_header_with_config(&DisplayConfig {
            decorative_header: false,
            ascii: false,
            spinner_enabled: false,
            spinner_threshold: slow_command_spinner_threshold(),
            command_elapsed: None,
            terminal_width: None,
        });

        assert!(
            rendered.is_empty(),
            "expected no header output, got: {rendered:?}"
        );
    }

    #[test]
    fn display_config_suppresses_header_by_default_when_stdout_is_not_a_tty() {
        let config = display_config_with(false, false, false, || false, || Some(72));

        assert_eq!(
            config,
            DisplayConfig {
                decorative_header: false,
                ascii: false,
                spinner_enabled: false,
                spinner_threshold: slow_command_spinner_threshold(),
                command_elapsed: None,
                terminal_width: Some(72),
            }
        );
    }

    #[test]
    fn display_config_keeps_header_by_default_when_stdout_is_a_tty() {
        let config = display_config_with(false, true, false, || true, || Some(72));

        assert_eq!(
            config,
            DisplayConfig {
                decorative_header: true,
                ascii: true,
                spinner_enabled: true,
                spinner_threshold: slow_command_spinner_threshold(),
                command_elapsed: None,
                terminal_width: Some(72),
            }
        );
    }

    #[test]
    fn header_rendering_adds_slow_command_status_for_real_display_path() {
        let rendered = strip_ansi(&render_header_with_config(&DisplayConfig {
            decorative_header: true,
            ascii: false,
            spinner_enabled: true,
            spinner_threshold: slow_command_spinner_threshold(),
            command_elapsed: Some(slow_command_spinner_threshold()),
            terminal_width: Some(30),
        }));

        assert!(rendered.contains("Port Whisperer"));
        assert!(
            rendered.contains("Working..."),
            "expected slow-command status line: {rendered}"
        );
    }

    #[test]
    fn header_rendering_skips_slow_command_status_when_spinner_is_disabled() {
        let rendered = strip_ansi(&render_header_with_config(&DisplayConfig {
            decorative_header: true,
            ascii: false,
            spinner_enabled: false,
            spinner_threshold: slow_command_spinner_threshold(),
            command_elapsed: Some(slow_command_spinner_threshold() + Duration::from_millis(1)),
            terminal_width: Some(30),
        }));

        assert!(
            !rendered.contains("Working..."),
            "did not expect slow-command status line: {rendered}"
        );
    }

    #[test]
    fn display_config_disables_spinner_for_quiet_json_and_non_interactive_output() {
        let quiet = display_config_with(true, false, false, || true, || Some(72));
        assert!(!quiet.spinner_enabled);

        let json = display_config_with(false, false, true, || true, || Some(72));
        assert!(!json.spinner_enabled);

        let non_interactive = display_config_with(false, false, false, || false, || Some(72));
        assert!(!non_interactive.spinner_enabled);
    }

    #[test]
    fn slow_command_spinner_threshold_only_triggers_for_slow_commands() {
        let config = display_config_with(false, false, false, || true, || Some(72));
        let threshold = slow_command_spinner_threshold();

        assert!(!should_show_slow_command_spinner(
            &config,
            threshold.saturating_sub(Duration::from_millis(1))
        ));
        assert!(should_show_slow_command_spinner(&config, threshold));
        assert!(should_show_slow_command_spinner(
            &config,
            threshold + Duration::from_millis(1)
        ));
    }

    #[test]
    fn clamp_header_width_applies_minimum_and_maximum_bounds() {
        assert_eq!(clamp_header_width(10), 20);
        assert_eq!(clamp_header_width(37), 37);
        assert_eq!(clamp_header_width(200), 80);
    }

    #[test]
    fn resolve_header_width_prefers_configured_width_over_detector() {
        assert_eq!(resolve_header_width(Some(52), || Some(72)), 52);
    }

    #[test]
    fn resolve_header_width_clamps_detected_terminal_width() {
        assert_eq!(resolve_header_width(None, || Some(12)), 20);
        assert_eq!(resolve_header_width(None, || Some(48)), 48);
        assert_eq!(resolve_header_width(None, || Some(120)), 80);
    }

    #[test]
    fn resolve_header_width_uses_default_when_terminal_width_is_unavailable() {
        assert_eq!(resolve_header_width(None, || None), 37);
    }

    #[test]
    fn detected_terminal_width_prefers_terminal_probe_and_falls_back_to_env() {
        assert_eq!(detected_terminal_width(|| Some(72), || Some(120)), Some(72));
        assert_eq!(detected_terminal_width(|| None, || Some(88)), Some(88));
        assert_eq!(detected_terminal_width(|| None, || None), None);
    }

    #[test]
    fn header_rendering_uses_configured_terminal_width() {
        let rendered = strip_ansi(&render_header_with_config(&DisplayConfig {
            decorative_header: true,
            ascii: false,
            spinner_enabled: true,
            spinner_threshold: slow_command_spinner_threshold(),
            command_elapsed: None,
            terminal_width: Some(20),
        }));

        let top_border = rendered
            .lines()
            .find(|line| line.contains('┌'))
            .expect("header top border should exist");

        assert_eq!(top_border, " ┌────────────────────┐");
    }

    #[test]
    fn header_rendering_uses_ascii_glyphs_when_requested() {
        let rendered = strip_ansi(&render_header_with_config(&DisplayConfig {
            decorative_header: true,
            ascii: true,
            spinner_enabled: true,
            spinner_threshold: slow_command_spinner_threshold(),
            command_elapsed: None,
            terminal_width: Some(20),
        }));

        assert!(
            rendered.contains("+") && !rendered.contains("┌"),
            "expected ascii header border: {rendered}"
        );
    }

    #[test]
    fn watch_event_line_uses_ascii_glyphs_when_requested() {
        let _guard = crate::style::glyph_test_lock().lock().unwrap();
        crate::style::set_force_ascii(true);
        let info = detailed_port_fixture();
        let rendered = strip_ansi(&render_watch_event_line("new", &info, "12:00:00"));
        crate::style::set_force_ascii(false);

        assert!(
            rendered.contains("<-"),
            "expected ascii arrow in watch event: {rendered}"
        );
        assert!(
            rendered.contains("^ NEW"),
            "expected ascii new marker in watch event: {rendered}"
        );
    }

    #[test]
    fn process_tree_line_uses_ascii_glyphs_when_requested() {
        let _guard = crate::style::glyph_test_lock().lock().unwrap();
        crate::style::set_force_ascii(true);
        let root = ProcessTreeNode {
            pid: 42,
            ppid: Some(1),
            name: "node".to_string(),
        };
        let child = ProcessTreeNode {
            pid: 1,
            ppid: None,
            name: "launchd".to_string(),
        };

        let root_line = strip_ansi(&render_process_tree_line(0, &root, 42));
        let child_line = strip_ansi(&render_process_tree_line(1, &child, 42));
        crate::style::set_force_ascii(false);

        assert!(
            root_line.contains("->"),
            "expected ascii root pointer: {root_line}"
        );
        assert!(
            child_line.contains("`-"),
            "expected ascii child branch: {child_line}"
        );
    }

    #[test]
    fn status_format_uses_ascii_glyphs_when_requested() {
        let _guard = crate::style::glyph_test_lock().lock().unwrap();
        crate::style::set_force_ascii(true);
        let rendered = strip_ansi(&format_status(&ProcessStatus::Healthy));
        crate::style::set_force_ascii(false);

        assert!(
            rendered.starts_with("* "),
            "expected ascii status marker: {rendered}"
        );
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
