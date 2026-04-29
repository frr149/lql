mod auth;
mod cli;
mod client;
mod commands;
mod config;
mod format;
mod middleware;
mod queries;

use clap::Parser;
use std::io::IsTerminal;
use std::process;

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    let effective_args = middleware::normalize_args(&raw_args).unwrap_or(raw_args.clone());
    let requested_json = effective_args.iter().any(|arg| arg == "--json");
    let machine_mode = requested_json || !std::io::stderr().is_terminal();
    if let Err(msg) = middleware::check_common_mistakes(&effective_args) {
        print_error(&msg, machine_mode);
        process::exit(1);
    }

    let args = cli::Cli::parse_from(&effective_args);
    cli::set_machine_mode(machine_mode || cli::command_prefers_machine_mode(&args.command));
    let config = match config::load() {
        Ok(c) => c,
        Err(e) => {
            print_error(&format!("Config error: {e}"), cli::machine_mode());
            process::exit(1);
        }
    };

    let result = match args.command {
        cli::Command::List(opts) => commands::list::run(&config, &opts),
        cli::Command::Create(opts) => commands::create::run(&config, &opts),
        cli::Command::Update(opts) => commands::update::run(&config, &opts),
        cli::Command::View(opts) => commands::view::run(&config, &opts),
        cli::Command::Search(opts) => commands::search::run(&config, &opts),
        cli::Command::Comment(opts) => commands::comment::run(&config, &opts),
        cli::Command::Relate(opts) => commands::relate::run(&config, &opts),
        cli::Command::Unlink(opts) => commands::relate::run_unlink(&config, &opts),
        cli::Command::Labels(opts) => commands::labels::run(&config, &opts),
        cli::Command::Doctor => commands::doctor::run(&config),
        cli::Command::Context => commands::context::run(&config),
        cli::Command::Epic(opts) => commands::epic::run(&config, &opts),
        cli::Command::Raw(opts) => commands::raw::run(&config, &opts),
    };

    if let Err(e) = result {
        print_error(&e.to_string(), cli::machine_mode());
        process::exit(1);
    }
}

fn print_error(message: &str, machine_mode: bool) {
    if machine_mode {
        eprintln!("error: {message}");
    } else {
        eprintln!("✗ {message}");
    }
}
