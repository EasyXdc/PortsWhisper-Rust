use std::process::Command;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchRequest {
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Platform {
    MacOs,
    Linux,
    Windows,
    Other(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchError {
    UnsupportedPlatform(String),
    CommandMissing(String),
    LaunchFailed(String),
}

pub fn run_open(port: u16) -> i32 {
    run_open_with(port, launch_in_default_browser)
}

pub fn run_open_with<F>(port: u16, launcher: F) -> i32
where
    F: FnOnce(&LaunchRequest) -> Result<(), LaunchError>,
{
    let request = LaunchRequest {
        url: format!("http://localhost:{port}"),
    };

    match launcher(&request) {
        Ok(()) => {
            println!("Opened {}", request.url);
            0
        }
        Err(LaunchError::UnsupportedPlatform(platform)) => {
            eprintln!("Opening a browser is not supported on this platform: {platform}");
            1
        }
        Err(LaunchError::CommandMissing(command)) => {
            eprintln!("Could not find a browser launcher command: {command}");
            1
        }
        Err(LaunchError::LaunchFailed(message)) => {
            eprintln!("Failed to open browser: {message}");
            1
        }
    }
}

fn launch_in_default_browser(request: &LaunchRequest) -> Result<(), LaunchError> {
    let command = launcher_command_for(current_platform(), &request.url)?;

    let status = Command::new(&command.program)
        .args(&command.args)
        .status()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                LaunchError::CommandMissing(command.program.clone())
            } else {
                LaunchError::LaunchFailed(format!("{}: {error}", command.program))
            }
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(LaunchError::LaunchFailed(format!(
            "{} exited with status {}",
            command.program,
            status.code().unwrap_or(-1)
        )))
    }
}

fn current_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Other(std::env::consts::OS.to_string())
    }
}

fn launcher_command_for(platform: Platform, url: &str) -> Result<LaunchCommand, LaunchError> {
    match platform {
        Platform::MacOs => Ok(LaunchCommand {
            program: "open".to_string(),
            args: vec![url.to_string()],
        }),
        Platform::Linux => Ok(LaunchCommand {
            program: "xdg-open".to_string(),
            args: vec![url.to_string()],
        }),
        Platform::Windows => Ok(LaunchCommand {
            program: "cmd".to_string(),
            args: vec![
                "/C".to_string(),
                "start".to_string(),
                "".to_string(),
                url.to_string(),
            ],
        }),
        Platform::Other(name) => Err(LaunchError::UnsupportedPlatform(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::{LaunchCommand, LaunchError, LaunchRequest, Platform, launcher_command_for, run_open_with};

    #[test]
    fn returns_success_when_launcher_accepts_localhost_url() {
        let mut seen = None;
        let exit_code = run_open_with(3000, |request| {
            seen = Some(request.clone());
            Ok(())
        });

        assert_eq!(exit_code, 0);
        assert_eq!(
            seen,
            Some(LaunchRequest {
                url: "http://localhost:3000".to_string(),
            })
        );
    }

    #[test]
    fn builds_macos_launcher_command() {
        assert_eq!(
            launcher_command_for(Platform::MacOs, "http://localhost:3000"),
            Ok(LaunchCommand {
                program: "open".to_string(),
                args: vec!["http://localhost:3000".to_string()],
            })
        );
    }

    #[test]
    fn builds_linux_launcher_command() {
        assert_eq!(
            launcher_command_for(Platform::Linux, "http://localhost:3000"),
            Ok(LaunchCommand {
                program: "xdg-open".to_string(),
                args: vec!["http://localhost:3000".to_string()],
            })
        );
    }

    #[test]
    fn builds_windows_launcher_command() {
        assert_eq!(
            launcher_command_for(Platform::Windows, "http://localhost:3000"),
            Ok(LaunchCommand {
                program: "cmd".to_string(),
                args: vec![
                    "/C".to_string(),
                    "start".to_string(),
                    "".to_string(),
                    "http://localhost:3000".to_string(),
                ],
            })
        );
    }

    #[test]
    fn rejects_unsupported_platform_for_launcher_command() {
        assert_eq!(
            launcher_command_for(Platform::Other("haiku".to_string()), "http://localhost:3000"),
            Err(LaunchError::UnsupportedPlatform("haiku".to_string()))
        );
    }

    #[test]
    fn reports_when_browser_launch_is_not_supported() {
        let exit_code = run_open_with(3000, |_| Err(LaunchError::UnsupportedPlatform("haiku".to_string())));

        assert_eq!(exit_code, 1);
    }

    #[test]
    fn reports_when_browser_launcher_command_is_missing() {
        let exit_code = run_open_with(3000, |_| Err(LaunchError::CommandMissing("xdg-open".to_string())));

        assert_eq!(exit_code, 1);
    }
}
