use crate::cli::CommentOpts;
use crate::client::Client;
use crate::commands::update::find_issue_by_identifier;
use crate::config::Config;
use std::io::{IsTerminal, Read};

pub fn run(config: &Config, opts: &CommentOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;

    // Resolver issue UUID
    let issue = find_issue_by_identifier(&client, &opts.issue_id)?;
    let issue_uuid = issue
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Could not get issue UUID")?;

    // Obtener body: inline > fichero > stdin
    let body = if let Some(ref text) = opts.body {
        text.clone()
    } else if let Some(ref path) = opts.file {
        std::fs::read_to_string(path)
            .map_err(|e| format!("Could not read file {path}: {e}"))?
    } else if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("Could not read from stdin: {e}"))?;
        buf
    } else {
        return Err("No comment body provided. Use: lql comment ID \"texto\" or --file or stdin".to_string());
    };

    if body.trim().is_empty() {
        return Err("Comment body is empty".to_string());
    }

    let variables = serde_json::json!({
        "input": {
            "issueId": issue_uuid,
            "body": body,
        }
    });

    let data = client.query(crate::queries::COMMENT_MUTATION, variables)?;

    let success = data
        .get("commentCreate")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if success {
        println!("✓ Comment added to {}", opts.issue_id);
    } else {
        return Err(format!("Failed to add comment to {}", opts.issue_id));
    }

    Ok(())
}
