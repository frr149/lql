use crate::cli::RelateOpts;
use crate::client::Client;
use crate::commands::update::find_issue_by_identifier;
use crate::config::Config;

pub fn run(config: &Config, opts: &RelateOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;

    // Normalizar tipo de relación
    let (relation_type, from_id, to_id, display_type) = match opts.relation_type.to_lowercase().as_str() {
        "blocks" => ("blocks", &opts.from, &opts.to, "blocks"),
        "blocked-by" | "blockedby" => {
            // Invertir: A blocked-by B → B blocks A
            ("blocks", &opts.to, &opts.from, "blocked-by")
        }
        "related" => ("related", &opts.from, &opts.to, "related"),
        other => {
            return Err(format!(
                "Unknown relation type \"{other}\". Available: blocks, blocked-by, related"
            ));
        }
    };

    // Resolver UUIDs
    let from_issue = find_issue_by_identifier(&client, from_id)?;
    let to_issue = find_issue_by_identifier(&client, to_id)?;

    let from_uuid = from_issue
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Could not get source issue UUID")?;
    let to_uuid = to_issue
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Could not get target issue UUID")?;

    let variables = serde_json::json!({
        "input": {
            "issueId": from_uuid,
            "relatedIssueId": to_uuid,
            "type": relation_type,
        }
    });

    let data = client.query(crate::queries::RELATION_MUTATION, variables)?;

    let success = data
        .get("issueRelationCreate")
        .and_then(|r| r.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if success {
        println!("✓ {} {display_type} {}", opts.from, opts.to);
    } else {
        return Err(format!(
            "Failed to create relation {} {display_type} {}",
            opts.from, opts.to
        ));
    }

    Ok(())
}
