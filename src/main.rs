mod auth;
mod cli;
mod client;
mod commands;
mod config;
mod format;
mod middleware;
mod queries;

use std::process;

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    if let Err(msg) = middleware::check_common_mistakes(&raw_args) {
        eprintln!("✗ {msg}");
        process::exit(1);
    }

    let args = cli::parse();
    let config = match config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("✗ Config error: {e}");
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
        cli::Command::Labels(opts) => commands::labels::run(&config, &opts),
        cli::Command::Doctor => commands::doctor::run(&config),
        cli::Command::Context => commands::context::run(&config),
        cli::Command::Raw(opts) => commands::raw::run(&config, &opts),
    };

    if let Err(e) = result {
        eprintln!("✗ {e}");
        process::exit(1);
    }
}
