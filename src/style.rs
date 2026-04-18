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
pub const BG_BLACK: &str = "\x1b[40m";

fn wrap(s: impl AsRef<str>, code: &str) -> String {
    if no_color() {
        return s.as_ref().to_string();
    }
    format!("{code}{}{RESET}", s.as_ref())
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some()
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

pub fn red_bold(s: impl AsRef<str>) -> String {
    if no_color() {
        return s.as_ref().to_string();
    }
    format!("{RED}{BOLD}{}{RESET}", s.as_ref())
}

pub fn framework(name: &str) -> String {
    if no_color() {
        return name.to_string();
    }
    match name {
        "Next.js" => format!("{WHITE}{BG_BLACK}{name}{RESET}"),
        "Vite" | "Python" | "esbuild" | "Elasticsearch" => yellow(name),
        "React" | "FastAPI" | "Go" => cyan(name),
        "Vue" | "Nuxt" | "Django" | "Node.js" | "MongoDB" | "nginx" => green(name),
        "Angular" | "NestJS" | "Rails" | "Ruby" | "Java" | "Redis" | "MinIO" => red(name),
        "Svelte" | "SvelteKit" | "Hono" | "RabbitMQ" => {
            "\x1b[38;2;255;102;0m".to_owned() + name + RESET
        }
        "Rust" => "\x1b[38;2;222;165;93m".to_owned() + name + RESET,
        "Parcel" => "\x1b[38;2;224;178;77m".to_owned() + name + RESET,
        "Express" => gray(name),
        "Remix" | "Docker" | "PostgreSQL" | "MySQL" | "Webpack" => blue(name),
        "Astro" | "Gatsby" => magenta(name),
        _ => white(name),
    }
}

#[cfg(test)]
mod tests {
    use super::{cyan_bold, gray};
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

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn restore_no_color(previous: Option<std::ffi::OsString>) {
        if let Some(value) = previous {
            unsafe { std::env::set_var("NO_COLOR", value) };
        } else {
            unsafe { std::env::remove_var("NO_COLOR") };
        }
    }
}
