pub mod auth;
pub mod cli;
pub mod client;
pub mod commands;
pub mod config;
pub mod format;
pub mod middleware;
pub mod queries;

use clap::Parser;
use std::io::IsTerminal;

/// Runs the lql CLI end to end and returns the process exit code.
///
/// Every module lives in the library crate (not the binary), so the unit
/// tests compile and run exactly once — in the lib test binary — instead of
/// being duplicated into a second `main.rs` test binary. `main.rs` is a thin
/// shell that just forwards here. See `docs/adr/0001-fast-iteration-builds.md`.
pub fn run() -> i32 {
    let raw_args: Vec<String> = std::env::args().collect();
    let effective_args = middleware::normalize_args(&raw_args).unwrap_or_else(|| raw_args.clone());
    let requested_json = effective_args.iter().any(|arg| arg == "--json");
    let machine_mode = requested_json || !std::io::stderr().is_terminal();
    if let Err(msg) = middleware::check_common_mistakes(&effective_args) {
        print_error(&msg, machine_mode);
        return 1;
    }

    let args = cli::Cli::parse_from(&effective_args);
    cli::set_machine_mode(machine_mode || cli::command_prefers_machine_mode(&args.command));
    let config = match config::load() {
        Ok(c) => c,
        Err(e) => {
            print_error(&format!("Config error: {e}"), cli::machine_mode());
            return 1;
        }
    };

    let result = match args.command {
        cli::Command::List(opts) => commands::list::run(&config, &opts),
        cli::Command::Create(opts) => commands::create::run(&config, &opts),
        cli::Command::Update(opts) => commands::update::run(&config, &opts),
        cli::Command::View(opts) => commands::view::run(&config, &opts),
        cli::Command::Search(opts) => commands::search::run(&config, &opts),
        cli::Command::Comment(opts) => commands::comment::run(&config, &opts),
        cli::Command::Comments(opts) => commands::view::run_comments(&config, &opts),
        cli::Command::Relate(opts) => commands::relate::run(&config, &opts),
        cli::Command::Unlink(opts) => commands::relate::run_unlink(&config, &opts),
        cli::Command::Labels(opts) => commands::labels::run(&config, &opts),
        cli::Command::Doctor => commands::doctor::run(&config),
        cli::Command::Context => commands::context::run(&config),
        cli::Command::Epic(opts) => commands::epic::run(&config, &opts),
        cli::Command::Project(opts) => commands::project::run(&config, &opts),
        cli::Command::Raw(opts) => commands::raw::run(&config, &opts),
    };

    if let Err(e) = result {
        print_error(&e.to_string(), cli::machine_mode());
        return 1;
    }
    0
}

fn print_error(message: &str, machine_mode: bool) {
    if machine_mode {
        eprintln!("error: {message}");
    } else {
        eprintln!("✗ {message}");
    }
}

/// Formats a warning line following the same machine/human convention as
/// `print_error`. Pure, so it can be asserted without capturing stderr.
fn warning_line(message: &str, machine_mode: bool) -> String {
    if machine_mode {
        format!("warning: {message}")
    } else {
        format!("\u{26a0} {message}")
    }
}

/// Emits a warning to **stderr** (never stdout, which carries the TOON/machine
/// payload). Used to announce implicit fallbacks such as the default team.
pub fn print_warning(message: &str, machine_mode: bool) {
    eprintln!("{}", warning_line(message, machine_mode));
}

#[cfg(test)]
mod tests {
    use super::*;

    // T01: warning line honors the machine/human convention.
    #[test]
    fn test_print_warning_machine_and_human_format() {
        assert_eq!(warning_line("no team", true), "warning: no team");
        assert_eq!(warning_line("no team", false), "\u{26a0} no team");
    }
}
