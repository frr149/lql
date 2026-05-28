use crate::cli::{CommentsOpts, ViewOpts};
use crate::client::Client;
use crate::commands::update::find_issue_by_identifier;
use crate::config::Config;
use crate::format;

pub fn run(config: &Config, opts: &ViewOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;
    let issue = find_issue_by_identifier(&client, &opts.issue_id)?;

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&issue).unwrap_or_default()
        );
    } else if opts.comments {
        println!("{}", format::format_comments(&issue));
    } else {
        println!("{}", format::format_view(&issue));
    }

    Ok(())
}

pub fn run_comments(config: &Config, opts: &CommentsOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;
    let issue = find_issue_by_identifier(&client, &opts.issue_id)?;

    if opts.json {
        let comments = issue
            .get("comments")
            .and_then(|c| c.get("nodes"))
            .cloned()
            .unwrap_or(serde_json::json!([]));
        println!(
            "{}",
            serde_json::to_string_pretty(&comments).unwrap_or_default()
        );
    } else {
        println!("{}", format::format_comments(&issue));
    }

    Ok(())
}
