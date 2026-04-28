pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";
pub const CYAN: &str = "\x1b[36m";
pub const WHITE: &str = "\x1b[37m";
pub const GRAY: &str = "\x1b[90m";

static FORCE_ASCII: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Glyphs {
    pub header_left_top: &'static str,
    pub header_right_top: &'static str,
    pub header_left_bottom: &'static str,
    pub header_right_bottom: &'static str,
    pub horizontal: &'static str,
    pub vertical: &'static str,
    pub table_top_left: &'static str,
    pub table_top_mid: &'static str,
    pub table_top_right: &'static str,
    pub table_mid_left: &'static str,
    pub table_mid_mid: &'static str,
    pub table_mid_right: &'static str,
    pub table_bottom_left: &'static str,
    pub table_bottom_mid: &'static str,
    pub table_bottom_right: &'static str,
    pub healthy: &'static str,
    pub orphaned: &'static str,
    pub zombie: &'static str,
    pub unknown: &'static str,
    pub success: &'static str,
    pub failure: &'static str,
    pub bullet: &'static str,
    pub new_label: &'static str,
    pub closed_label: &'static str,
    pub watch_arrow: &'static str,
    pub tree_root: &'static str,
    pub tree_branch: &'static str,
    pub logs_pointer: &'static str,
    pub speaker: &'static str,
}

impl Glyphs {
    pub const fn unicode() -> Self {
        Self {
            header_left_top: "┌",
            header_right_top: "┐",
            header_left_bottom: "└",
            header_right_bottom: "┘",
            horizontal: "─",
            vertical: "│",
            table_top_left: "┌",
            table_top_mid: "┬",
            table_top_right: "┐",
            table_mid_left: "├",
            table_mid_mid: "┼",
            table_mid_right: "┤",
            table_bottom_left: "└",
            table_bottom_mid: "┴",
            table_bottom_right: "┘",
            healthy: "●",
            orphaned: "●",
            zombie: "●",
            unknown: "●",
            success: "✓",
            failure: "✕",
            bullet: "•",
            new_label: "▲ NEW",
            closed_label: "▼ CLOSED",
            watch_arrow: "←",
            tree_root: "→",
            tree_branch: "└─",
            logs_pointer: "▸",
            speaker: "🔊",
        }
    }

    pub const fn ascii() -> Self {
        Self {
            header_left_top: "+",
            header_right_top: "+",
            header_left_bottom: "+",
            header_right_bottom: "+",
            horizontal: "-",
            vertical: "|",
            table_top_left: "+",
            table_top_mid: "+",
            table_top_right: "+",
            table_mid_left: "+",
            table_mid_mid: "+",
            table_mid_right: "+",
            table_bottom_left: "+",
            table_bottom_mid: "+",
            table_bottom_right: "+",
            healthy: "*",
            orphaned: "*",
            zombie: "*",
            unknown: "*",
            success: "v",
            failure: "x",
            bullet: "*",
            new_label: "^ NEW",
            closed_label: "v CLOSED",
            watch_arrow: "<-",
            tree_root: "->",
            tree_branch: "`-",
            logs_pointer: "->",
            speaker: "Port",
        }
    }
}

pub fn set_force_ascii(force: bool) {
    FORCE_ASCII.store(force, std::sync::atomic::Ordering::Relaxed);
}

pub fn glyphs() -> Glyphs {
    glyphs_for(FORCE_ASCII.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn glyphs_for(force_ascii: bool) -> Glyphs {
    if force_ascii || dumb_terminal() {
        Glyphs::ascii()
    } else {
        Glyphs::unicode()
    }
}

fn dumb_terminal() -> bool {
    std::env::var("TERM")
        .ok()
        .is_some_and(|term| term.eq_ignore_ascii_case("dumb"))
}

#[cfg(test)]
pub fn glyph_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

fn wrap(s: impl AsRef<str>, code: &str) -> String {
    if no_color() {
        return s.as_ref().to_string();
    }
    format!("{}{s}{RESET}", adjusted_code(code), s = s.as_ref())
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

fn adjusted_code(code: &str) -> &str {
    if code == GRAY && prefers_low_contrast_safe_text() {
        WHITE
    } else {
        code
    }
}

fn prefers_low_contrast_safe_text() -> bool {
    matches!(
        std::env::var("TERM").ok().as_deref(),
        Some("vt100" | "vt220" | "ansi")
    ) && std::env::var_os("COLORTERM").is_none()
}

pub fn bold(s: impl AsRef<str>) -> String {
    wrap(s, BOLD)
}

pub fn dim(s: impl AsRef<str>) -> String {
    wrap(s, DIM)
}

pub fn red(s: impl AsRef<str>) -> String {
    wrap(s, RED)
}

pub fn green(s: impl AsRef<str>) -> String {
    wrap(s, GREEN)
}

pub fn yellow(s: impl AsRef<str>) -> String {
    wrap(s, YELLOW)
}

pub fn blue(s: impl AsRef<str>) -> String {
    wrap(s, BLUE)
}

pub fn magenta(s: impl AsRef<str>) -> String {
    wrap(s, MAGENTA)
}

pub fn cyan(s: impl AsRef<str>) -> String {
    wrap(s, CYAN)
}

pub fn white(s: impl AsRef<str>) -> String {
    wrap(s, WHITE)
}

pub fn gray(s: impl AsRef<str>) -> String {
    wrap(s, GRAY)
}

pub fn cyan_bold(s: impl AsRef<str>) -> String {
    if no_color() {
        return s.as_ref().to_string();
    }
    format!("{CYAN}{BOLD}{}{RESET}", s.as_ref())
}

pub fn white_bold(s: impl AsRef<str>) -> String {
    if no_color() {
        return s.as_ref().to_string();
    }
    format!("{WHITE}{BOLD}{}{RESET}", s.as_ref())
}

pub fn yellow_bold(s: impl AsRef<str>) -> String {
    if no_color() {
        return s.as_ref().to_string();
    }
    format!("{YELLOW}{BOLD}{}{RESET}", s.as_ref())
}

pub fn framework(name: &str) -> String {
    if no_color() {
        return name.to_string();
    }
    let metadata = crate::framework::display_metadata(name);
    format!("{}{name}{RESET}", metadata.ansi_prefix)
}

#[cfg(test)]
mod tests {
    use super::{Glyphs, cyan_bold, glyphs_for, gray};
    use crate::framework;
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn no_color_disables_ansi_wrapping() {
        let _guard = env_lock().lock().unwrap();
        let previous = std::env::var_os("NO_COLOR");
        unsafe { std::env::set_var("NO_COLOR", "1") };

        assert_eq!(gray("plain"), "plain");
        assert_eq!(cyan_bold("title"), "title");

        restore_no_color(previous);
    }

    #[test]
    fn glyphs_use_unicode_by_default() {
        let _guard = env_lock().lock().unwrap();
        let previous_term = std::env::var_os("TERM");
        unsafe { std::env::remove_var("TERM") };

        assert_eq!(glyphs_for(false), Glyphs::unicode());

        restore_env("TERM", previous_term);
    }

    #[test]
    fn glyphs_force_ascii_when_requested() {
        assert_eq!(glyphs_for(true), Glyphs::ascii());
    }

    #[test]
    fn glyphs_fall_back_to_ascii_for_dumb_term() {
        let _guard = env_lock().lock().unwrap();
        let previous_term = std::env::var_os("TERM");
        unsafe { std::env::set_var("TERM", "dumb") };

        assert_eq!(glyphs_for(false), Glyphs::ascii());

        restore_env("TERM", previous_term);
    }

    #[test]
    fn gray_uses_white_when_terminal_lacks_bright_color_support() {
        let _guard = env_lock().lock().unwrap();
        let previous_term = std::env::var_os("TERM");
        let previous_colorterm = std::env::var_os("COLORTERM");
        let previous_no_color = std::env::var_os("NO_COLOR");
        unsafe {
            std::env::set_var("TERM", "vt100");
            std::env::remove_var("COLORTERM");
            std::env::remove_var("NO_COLOR");
        };

        assert_eq!(
            gray("plain"),
            format!("{}plain{}", super::WHITE, super::RESET)
        );

        restore_env("TERM", previous_term);
        restore_env("COLORTERM", previous_colorterm);
        restore_env("NO_COLOR", previous_no_color);
    }

    #[test]
    fn framework_styles_use_display_metadata() {
        assert_eq!(
            super::framework("Next.js"),
            format!(
                "{}{}{}",
                framework::display_metadata("Next.js").ansi_prefix,
                "Next.js",
                super::RESET
            )
        );
        assert_eq!(
            super::framework("SvelteKit"),
            format!(
                "{}{}{}",
                framework::display_metadata("SvelteKit").ansi_prefix,
                "SvelteKit",
                super::RESET
            )
        );
        assert_eq!(
            super::framework("Unknown"),
            format!(
                "{}{}{}",
                framework::display_metadata("Unknown").ansi_prefix,
                "Unknown",
                super::RESET
            )
        );
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn restore_no_color(previous: Option<std::ffi::OsString>) {
        restore_env("NO_COLOR", previous);
    }

    fn restore_env(name: &str, previous: Option<std::ffi::OsString>) {
        if let Some(value) = previous {
            unsafe { std::env::set_var(name, value) };
        } else {
            unsafe { std::env::remove_var(name) };
        }
    }
}
