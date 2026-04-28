//! Port Whisperer — a CLI for discovering, inspecting, and managing local port listeners.
//!
//! This crate provides the core library logic shared between the `ports` and
//! `whoisonport` binaries. The main entry point is [`run_app`].

pub mod check;
pub mod cli;
mod cli_args;
pub mod display;
pub mod docker;
pub mod error;
pub mod framework;
pub mod json_output;
pub mod kill;
pub mod logs;
pub mod model;
pub mod open;
pub mod platform;
pub mod ports;
pub mod process;
pub mod scanner;
pub mod style;
#[cfg(test)]
pub mod test_support;
pub mod util;
pub mod watch;

/// Run the CLI application with the given binary name and arguments, returning an exit code.
pub fn run_app(binary_name: &str, args: Vec<String>) -> i32 {
    cli::run(binary_name, args)
}
