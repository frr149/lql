use crate::cli::{self, UpdateOpts};
use crate::client::{Client, LinearMeta};
use crate::config::Config;
use crate::format;

pub fn run(config: &Config, opts: &UpdateOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;
    let meta = LinearMeta::fetch(&client)?;

    // Resolver issue por identifier
    let issue = find_issue_by_identifier(&client, &opts.issue_id)?;
    let issue_uuid = issue
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Could not get issue UUID")?;
    let old_state = issue
        .get("state")
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("Unknown");
    let team_key = issue
        .get("team")
        .and_then(|t| t.get("key"))
        .and_then(|k| k.as_str())
        .ok_or("Issue has no team")?;

    let team = meta.find_team(team_key)?;

    // Construir input
    let mut input = serde_json::json!({});
    let mut has_changes = false;

    // Estado
    let mut new_state_name = old_state.to_string();
    if let Some(ref state_str) = opts.state {
        let state_type = cli::normalize_state(state_str, &config.state_aliases);
        if let Some(state) = meta.find_state(team, &state_type) {
            input["stateId"] = serde_json::json!(state.id);
            new_state_name = state.name.clone();
            has_changes = true;
        }
    }

    // Prioridad
    if let Some(ref prio) = opts.priority {
        let p = cli::normalize_priority(prio, &config.priority_aliases)?;
        input["priority"] = serde_json::json!(p);
        has_changes = true;
    }

    // Proyecto
    if let Some(ref project_name) = opts.project {
        let project = meta.find_project(team, project_name)?;
        input["projectId"] = serde_json::json!(project.id);
        has_changes = true;
    }

    // Labels (additive)
    if let Some(ref label_names) = opts.label {
        // Obtener labels actuales
        let current_labels: Vec<String> = issue
            .get("labels")
            .and_then(|l| l.get("nodes"))
            .and_then(|n| n.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Resolver todos los labels (actuales + nuevos)
        let mut all_label_ids = Vec::new();
        for name in &current_labels {
            if let Ok(label) = meta.find_label(name) {
                all_label_ids.push(serde_json::json!(label.id));
            }
        }
        for name in label_names {
            let label = meta.find_label(name)?;
            all_label_ids.push(serde_json::json!(label.id));
        }
        input["labelIds"] = serde_json::json!(all_label_ids);
        has_changes = true;
    }

    // Título
    if let Some(ref title) = opts.title {
        input["title"] = serde_json::json!(title);
        has_changes = true;
    }

    // Descripción
    if let Some(ref desc) = opts.description {
        input["description"] = serde_json::json!(desc);
        has_changes = true;
    }
    if let Some(ref path) = opts.description_file {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Could not read description file {path}: {e}"))?;
        input["description"] = serde_json::json!(content);
        has_changes = true;
    }

    // Due date
    if let Some(ref due) = opts.due {
        let date = crate::commands::create::parse_due_date_pub(due)?;
        input["dueDate"] = serde_json::json!(date);
        has_changes = true;
    }

    if !has_changes {
        return Err("No changes specified. Use --state, --priority, --label, --title, --project, or --due.".to_string());
    }

    let variables = serde_json::json!({
        "id": issue_uuid,
        "input": input,
    });

    let data = client.query(crate::queries::UPDATE_MUTATION, variables)?;

    let success = data
        .get("issueUpdate")
        .and_then(|u| u.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if !success {
        return Err(format!("Failed to update {}", opts.issue_id));
    }

    if opts.json {
        if let Some(updated) = data.get("issueUpdate").and_then(|u| u.get("issue")) {
            println!("{}", format::format_issue_json(updated));
        }
    } else {
        println!(
            "{}",
            format::format_updated(&opts.issue_id, old_state, &new_state_name)
        );
    }

    Ok(())
}

/// Busca una issue por identifier (PROD-587) y devuelve los datos
pub fn find_issue_by_identifier(
    client: &Client,
    identifier: &str,
) -> Result<serde_json::Value, String> {
    // Parsear team key y number del identifier
    let (team_key, number) = parse_identifier(identifier)?;

    let filter = serde_json::json!({
        "team": {"key": {"eq": team_key}},
        "number": {"eq": number},
    });

    let variables = serde_json::json!({"filter": filter});
    let data = client.query(crate::queries::ISSUE_BY_IDENTIFIER, variables)?;

    let issues = data
        .get("issues")
        .and_then(|i| i.get("nodes"))
        .and_then(|n| n.as_array())
        .ok_or_else(|| format!("{identifier} not found."))?;

    issues
        .first()
        .cloned()
        .ok_or_else(|| format!("{identifier} not found."))
}

/// Parsea un identifier (PROD-587) en (team_key, number)
fn parse_identifier(identifier: &str) -> Result<(String, u32), String> {
    let parts: Vec<&str> = identifier.splitn(2, '-').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid issue identifier \"{identifier}\". Expected format: TEAM-123"
        ));
    }
    let team_key = parts[0].to_uppercase();
    let number: u32 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid issue number in \"{identifier}\""))?;
    Ok((team_key, number))
}
