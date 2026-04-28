use crate::util::basename;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameworkDisplayMetadata {
    pub ansi_prefix: &'static str,
}

pub fn display_metadata(name: &str) -> FrameworkDisplayMetadata {
    match name {
        "Next.js" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[37m\x1b[40m",
        },
        "Vite" | "Python" | "esbuild" | "Elasticsearch" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[33m",
        },
        "React" | "FastAPI" | "Go" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[36m",
        },
        "Vue" | "Nuxt" | "Django" | "Node.js" | "MongoDB" | "nginx" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[32m",
        },
        "Angular" | "NestJS" | "Rails" | "Ruby" | "Java" | "Redis" | "MinIO" => {
            FrameworkDisplayMetadata {
                ansi_prefix: "\x1b[31m",
            }
        }
        "Svelte" | "SvelteKit" | "Hono" | "RabbitMQ" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[38;2;255;102;0m",
        },
        "Rust" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[38;2;222;165;93m",
        },
        "Parcel" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[38;2;224;178;77m",
        },
        "Express" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[90m",
        },
        "Remix" | "Docker" | "PostgreSQL" | "MySQL" | "Webpack" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[34m",
        },
        "Astro" | "Gatsby" => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[35m",
        },
        _ => FrameworkDisplayMetadata {
            ansi_prefix: "\x1b[37m",
        },
    }
}

pub fn is_dev_process(process_name: &str, command: &str) -> bool {
    let name = process_name.to_lowercase();
    let cmd = command.to_lowercase();
    let system_apps = [
        "spotify",
        "raycast",
        "tableplus",
        "postman",
        "linear",
        "cursor",
        "controlce",
        "rapportd",
        "superhuma",
        "setappage",
        "slack",
        "discord",
        "firefox",
        "chrome",
        "google",
        "safari",
        "figma",
        "notion",
        "zoom",
        "teams",
        "code",
        "iterm2",
        "warp",
        "arc",
        "loginwindow",
        "windowserver",
        "systemuise",
        "kernel_task",
        "launchd",
        "mdworker",
        "mds_stores",
        "cfprefsd",
        "coreaudio",
        "corebrightne",
        "airportd",
        "bluetoothd",
        "sharingd",
        "usernoted",
        "notificationc",
        "cloudd",
        "systemd",
        "snapd",
        "networkmanager",
        "gdm",
        "sshd",
        "cron",
        "dbus-daemon",
        "polkitd",
        "rsyslogd",
        "thermald",
        "accounts-daemon",
        "svchost",
        "csrss",
        "lsass",
        "services",
        "explorer",
        "dwm",
        "searchindexer",
        "taskhostw",
        "runtimebroker",
        "shellexperiencehost",
    ];
    if system_apps.iter().any(|app| name.starts_with(app)) {
        return false;
    }
    let dev_names = [
        "node",
        "python",
        "python3",
        "ruby",
        "java",
        "go",
        "cargo",
        "deno",
        "bun",
        "php",
        "uvicorn",
        "gunicorn",
        "flask",
        "rails",
        "npm",
        "npx",
        "yarn",
        "pnpm",
        "tsc",
        "tsx",
        "esbuild",
        "rollup",
        "turbo",
        "nx",
        "jest",
        "vitest",
        "mocha",
        "pytest",
        "cypress",
        "playwright",
        "rustc",
        "dotnet",
        "gradle",
        "mvn",
        "mix",
        "elixir",
    ];
    if dev_names.contains(&name.as_str()) || is_docker_process(&name) {
        return true;
    }
    let indicators = [
        "node",
        "next ",
        "next-",
        "vite",
        "nuxt",
        "webpack",
        "remix",
        "astro",
        "gulp",
        "ng serve",
        "gatsb",
        "flask",
        "django",
        "manage.py",
        "uvicorn",
        "rails",
        "cargo",
    ];
    indicators
        .iter()
        .any(|needle| contains_wordish(&cmd, needle))
}

pub fn is_docker_process(name: &str) -> bool {
    name.starts_with("com.docke")
        || name.starts_with("Docker")
        || name == "docker"
        || name == "docker-sandbox"
}

pub fn detect_framework(project_root: &Path) -> Option<String> {
    let pkg_path = project_root.join("package.json");
    if let Ok(pkg) = std::fs::read_to_string(pkg_path) {
        let checks = [
            ("\"next\"", "Next.js"),
            ("\"nuxt\"", "Nuxt"),
            ("\"nuxt3\"", "Nuxt"),
            ("\"@sveltejs/kit\"", "SvelteKit"),
            ("\"svelte\"", "Svelte"),
            ("\"@remix-run/react\"", "Remix"),
            ("\"remix\"", "Remix"),
            ("\"astro\"", "Astro"),
            ("\"vite\"", "Vite"),
            ("\"@angular/core\"", "Angular"),
            ("\"vue\"", "Vue"),
            ("\"react\"", "React"),
            ("\"express\"", "Express"),
            ("\"fastify\"", "Fastify"),
            ("\"hono\"", "Hono"),
            ("\"koa\"", "Koa"),
            ("\"nestjs\"", "NestJS"),
            ("\"@nestjs/core\"", "NestJS"),
            ("\"gatsby\"", "Gatsby"),
            ("\"webpack-dev-server\"", "Webpack"),
            ("\"esbuild\"", "esbuild"),
            ("\"parcel\"", "Parcel"),
        ];
        for (needle, framework) in checks {
            if pkg.contains(needle) {
                return Some(framework.to_string());
            }
        }
    }

    let marker_checks = [
        ("vite.config.ts", "Vite"),
        ("vite.config.js", "Vite"),
        ("next.config.js", "Next.js"),
        ("next.config.mjs", "Next.js"),
        ("angular.json", "Angular"),
        ("Cargo.toml", "Rust"),
        ("go.mod", "Go"),
        ("manage.py", "Django"),
        ("Gemfile", "Ruby"),
    ];
    for (marker, framework) in marker_checks {
        if project_root.join(marker).exists() {
            return Some(framework.to_string());
        }
    }
    None
}

pub fn detect_framework_from_command(command: &str, process_name: &str) -> Option<String> {
    let cmd = command.to_lowercase();
    let checks = [
        ("next", "Next.js"),
        ("vite", "Vite"),
        ("nuxt", "Nuxt"),
        ("angular", "Angular"),
        ("ng serve", "Angular"),
        ("webpack", "Webpack"),
        ("remix", "Remix"),
        ("astro", "Astro"),
        ("gatsby", "Gatsby"),
        ("flask", "Flask"),
        ("django", "Django"),
        ("manage.py", "Django"),
        ("uvicorn", "FastAPI"),
        ("rails", "Rails"),
        ("cargo", "Rust"),
        ("rustc", "Rust"),
    ];
    for (needle, framework) in checks {
        if cmd.contains(needle) {
            return Some(framework.to_string());
        }
    }
    detect_framework_from_name(process_name)
}

pub fn detect_framework_from_name(process_name: &str) -> Option<String> {
    match process_name.to_lowercase().as_str() {
        "node" => Some("Node.js"),
        "python" | "python3" => Some("Python"),
        "ruby" => Some("Ruby"),
        "java" => Some("Java"),
        "go" => Some("Go"),
        _ => None,
    }
    .map(str::to_string)
}

pub fn detect_framework_from_image(image: &str) -> String {
    let img = image.to_lowercase();
    if img.contains("postgres") {
        "PostgreSQL"
    } else if img.contains("redis") {
        "Redis"
    } else if img.contains("mysql") || img.contains("mariadb") {
        "MySQL"
    } else if img.contains("mongo") {
        "MongoDB"
    } else if img.contains("nginx") {
        "nginx"
    } else if img.contains("localstack") {
        "LocalStack"
    } else if img.contains("rabbitmq") {
        "RabbitMQ"
    } else if img.contains("kafka") {
        "Kafka"
    } else if img.contains("elasticsearch") || img.contains("opensearch") {
        "Elasticsearch"
    } else if img.contains("minio") {
        "MinIO"
    } else {
        "Docker"
    }
    .to_string()
}

pub fn summarize_command(command: &str, process_name: &str) -> String {
    let mut meaningful = Vec::new();
    for (idx, part) in command.split_whitespace().enumerate() {
        if idx == 0 || part.starts_with('-') {
            continue;
        }
        if part.contains('/') {
            meaningful.push(basename(part));
        } else {
            meaningful.push(part.to_string());
        }
        if meaningful.len() >= 3 {
            break;
        }
    }
    if meaningful.is_empty() {
        process_name.to_string()
    } else {
        meaningful.join(" ")
    }
}

fn contains_wordish(haystack: &str, needle: &str) -> bool {
    if needle.contains(' ') || needle.ends_with('-') || needle.ends_with(' ') {
        return haystack.contains(needle);
    }
    haystack
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '.')
        .any(|part| part == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn dev_process_rules_match_reference_names_and_filters() {
        assert!(is_dev_process("node", "/usr/local/bin/node server.js"));
        assert!(is_dev_process("python3", "python3 -m uvicorn app:app"));
        assert!(is_dev_process("com.docker.backend", ""));
        assert!(is_dev_process("anything", "npm run vite -- --host"));
        assert!(!is_dev_process("Spotify", ""));
        assert!(!is_dev_process("rapportd", ""));
    }

    #[test]
    fn framework_detection_from_command_and_name_matches_reference() {
        assert_eq!(
            detect_framework_from_command("npm run next dev", "node").as_deref(),
            Some("Next.js")
        );
        assert_eq!(
            detect_framework_from_command("python -m uvicorn app:app", "python3").as_deref(),
            Some("FastAPI")
        );
        assert_eq!(
            detect_framework_from_command("", "ruby").as_deref(),
            Some("Ruby")
        );
    }

    #[test]
    fn docker_image_detection_matches_reference() {
        assert_eq!(detect_framework_from_image("postgres:16"), "PostgreSQL");
        assert_eq!(detect_framework_from_image("mariadb:latest"), "MySQL");
        assert_eq!(
            detect_framework_from_image("opensearchproject/opensearch"),
            "Elasticsearch"
        );
        assert_eq!(detect_framework_from_image(""), "Docker");
    }

    #[test]
    fn package_and_marker_framework_detection_work() {
        let root = temp_project_dir("framework");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"@sveltejs/kit":"latest"}}"#,
        )
        .unwrap();
        assert_eq!(detect_framework(&root).as_deref(), Some("SvelteKit"));

        fs::remove_file(root.join("package.json")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert_eq!(detect_framework(&root).as_deref(), Some("Rust"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn command_summary_skips_binary_and_flags_then_truncates_to_three_parts() {
        assert_eq!(
            summarize_command("node --inspect /repo/server.js --port 3000 extra", "node"),
            "server.js 3000 extra"
        );
        assert_eq!(summarize_command("node --watch", "node"), "node");
    }

    #[test]
    fn display_metadata_returns_expected_prefixes() {
        assert_eq!(
            display_metadata("Next.js"),
            FrameworkDisplayMetadata {
                ansi_prefix: "\x1b[37m\x1b[40m",
            }
        );
        assert_eq!(
            display_metadata("SvelteKit"),
            FrameworkDisplayMetadata {
                ansi_prefix: "\x1b[38;2;255;102;0m",
            }
        );
        assert_eq!(
            display_metadata("Unknown"),
            FrameworkDisplayMetadata {
                ansi_prefix: "\x1b[37m",
            }
        );
    }

    use crate::test_support::temp_project_dir;
}
