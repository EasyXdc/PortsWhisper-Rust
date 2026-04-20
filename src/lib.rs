pub mod cli;
pub mod display;
pub mod docker;
pub mod error;
pub mod framework;
pub mod kill;
pub mod logs;
pub mod model;
pub mod platform;
pub mod ports;
pub mod process;
pub mod scanner;
pub mod style;
#[cfg(test)]
pub mod test_support;
pub mod util;
pub mod watch;

pub fn run_app(binary_name: &str, args: Vec<String>) -> i32 {
    cli::run(binary_name, args)
}
