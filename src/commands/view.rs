use crate::cli::ViewOpts;
use crate::client::{Client, GraphQLClient};
use crate::commands::update::find_issue_by_identifier;
use crate::config::Config;
use crate::format;

pub fn run(config: &Config, opts: &ViewOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;
    let issue = find_issue_by_identifier(&client, &opts.issue_id)?;

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&issue).unwrap_or_default()
        );
    } else {
        println!("{}", format::format_view(&issue));
    }

    Ok(())
}
