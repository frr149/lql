use crate::cli::{self, UpdateOpts};
use crate::client::{Client, GraphQLClient, LinearMeta};
use crate::config::Config;
use crate::format;

pub fn run(config: &Config, opts: &UpdateOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;
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

    // Move to different team
    if let Some(ref target_team_key) = opts.team {
        let target_team = meta.find_team(target_team_key)?;
        input["teamId"] = serde_json::json!(target_team.id);
        has_changes = true;
    }

    // Estado
    let mut new_state_name = old_state.to_string();
    if let Some(ref state_str) = opts.state {
        let effective_team = if let Some(ref target_key) = opts.team {
            meta.find_team(target_key)?
        } else {
            team
        };
        let state =
            meta.find_state_for_mutation(effective_team, state_str, &config.state_aliases)?;
        input["stateId"] = serde_json::json!(state.id);
        new_state_name = state.name.clone();
        has_changes = true;
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

    // Labels (additive). Current labels are preserved by ID. Re-resolving them by
    // name would be lossy: any name that failed to resolve would be silently
    // dropped from the replacement set, deleting a label the user never touched
    // (issueUpdate.labelIds replaces, it does not append). See /types V8b.
    if let Some(ref label_names) = opts.label {
        let mut all_label_ids: Vec<serde_json::Value> = issue
            .get("labels")
            .and_then(|l| l.get("nodes"))
            .and_then(|n| n.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|l| l.get("id").and_then(|i| i.as_str()))
                    .map(|id| serde_json::json!(id))
                    .collect()
            })
            .unwrap_or_default();

        for name in label_names {
            let label = meta.find_label_for_team(team, name)?;
            let id = serde_json::json!(label.id);
            if !all_label_ids.contains(&id) {
                all_label_ids.push(id);
            }
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
        return Err(
            "No changes specified. Use --state, --priority, --label, --title, --project, --team, or --due."
                .to_string(),
        );
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
    client: &dyn GraphQLClient,
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
pub fn parse_identifier(identifier: &str) -> Result<(String, u32), String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::GraphQLClient;

    struct MockClient {
        response: Result<serde_json::Value, String>,
    }

    impl GraphQLClient for MockClient {
        fn query(
            &self,
            _query: &str,
            _variables: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            self.response.clone()
        }
    }

    // --- Identifier parsing ---

    #[test]
    fn test_parse_identifier_valid() {
        let (team, num) = parse_identifier("PROD-587").unwrap();
        assert_eq!(team, "PROD");
        assert_eq!(num, 587);
    }

    #[test]
    fn test_parse_identifier_lowercase() {
        let (team, num) = parse_identifier("prod-587").unwrap();
        assert_eq!(team, "PROD");
        assert_eq!(num, 587);
    }

    #[test]
    fn test_parse_identifier_invalid_no_dash() {
        assert!(parse_identifier("PROD587").is_err());
    }

    #[test]
    fn test_parse_identifier_invalid_no_number() {
        assert!(parse_identifier("PROD-abc").is_err());
    }

    #[test]
    fn test_parse_identifier_invalid_empty() {
        assert!(parse_identifier("").is_err());
    }

    #[test]
    fn test_parse_identifier_tool_team() {
        let (team, num) = parse_identifier("TOOL-33").unwrap();
        assert_eq!(team, "TOOL");
        assert_eq!(num, 33);
    }

    // --- ERR-53: issue no encontrada (API devuelve nodes vacío) ---
    #[test]
    fn test_find_issue_not_found_empty_nodes() {
        let client = MockClient {
            response: Ok(serde_json::json!({
                "issues": {"nodes": []}
            })),
        };
        let err = find_issue_by_identifier(&client, "PROD-99999").unwrap_err();
        assert!(err.contains("PROD-99999 not found"), "{err}");
    }

    // ERR-53b: API devuelve error GraphQL
    #[test]
    fn test_find_issue_api_error() {
        let client = MockClient {
            response: Err("Linear API error: Entity not found".to_string()),
        };
        let err = find_issue_by_identifier(&client, "PROD-99999").unwrap_err();
        assert!(err.contains("Entity not found"), "{err}");
    }

    // ERR-53c: API devuelve estructura inesperada
    #[test]
    fn test_find_issue_malformed_response() {
        let client = MockClient {
            response: Ok(serde_json::json!({"something": "else"})),
        };
        let err = find_issue_by_identifier(&client, "PROD-587").unwrap_err();
        assert!(err.contains("not found"), "{err}");
    }

    // --- FIXED BUG: `--state` resolution by display name + category collision ---
    //
    // Was docs/bugs/update-state-ignored-no-changes.md (2026-06-18). The fix is
    // `LinearMeta::find_state_for_mutation`: display name wins over category, an
    // ambiguous category is a hard error (never a silent first-of-category pick),
    // and an unresolvable input lists the available states (never silent-dropped).
    // See /types V8 (conflación de conceptos).
    #[test]
    fn test_find_state_for_mutation_name_wins_and_ambiguity_errors() {
        use crate::client::{LinearMeta, StateInfo, TeamInfo};
        use std::collections::HashMap;

        // A team with TWO started states and TWO canceled states — exactly the
        // shape that made the old category-based resolver pick the wrong one.
        let st = |id: &str, name: &str, ty: &str| StateInfo {
            id: id.into(),
            name: name.into(),
            state_type: ty.into(),
        };
        let team = TeamInfo {
            id: "team-uuid".into(),
            key: "PROD".into(),
            name: "Product".into(),
            states: vec![
                st("st-backlog", "Backlog", "backlog"),
                st("st-review", "In Review", "started"),
                st("st-progress", "In Progress", "started"),
                // "Duplicate" is listed BEFORE "Canceled": a `.first()`-by-category
                // resolver would wrongly return it for `--state "Canceled"`.
                st("st-duplicate", "Duplicate", "canceled"),
                st("st-canceled", "Canceled", "canceled"),
            ],
            projects: vec![],
        };
        let meta = LinearMeta {
            teams: vec![team.clone()],
            labels: vec![],
        };
        let aliases: HashMap<String, String> = HashMap::from([
            ("Todo".into(), "unstarted".into()),
            ("In Progress".into(), "started".into()),
            ("Done".into(), "completed".into()),
            ("Canceled".into(), "canceled".into()),
            ("Cancelled".into(), "canceled".into()),
        ]);

        // The original bug: a custom display name now resolves (no silent drop).
        let review = meta
            .find_state_for_mutation(&team, "In Review", &aliases)
            .expect("custom display name must resolve");
        assert_eq!(review.id, "st-review");

        // The collision: "Canceled" the NAME must win over "Duplicate" (same
        // category, listed first) — never write the wrong state.
        let canceled = meta
            .find_state_for_mutation(&team, "Canceled", &aliases)
            .expect("exact name must resolve");
        assert_eq!(canceled.id, "st-canceled");
        assert_eq!(canceled.name, "Canceled");

        // A bare category with >1 member is a hard error, not a silent pick.
        let err = meta
            .find_state_for_mutation(&team, "started", &aliases)
            .unwrap_err();
        assert!(err.contains("ambiguous"), "{err}");

        // An unresolvable input lists the available states.
        let err = meta
            .find_state_for_mutation(&team, "Nonexistent", &aliases)
            .unwrap_err();
        assert!(
            err.contains("not found") && err.contains("Backlog"),
            "{err}"
        );
    }

    // ERR-54: issue encontrada exitosamente
    #[test]
    fn test_find_issue_success() {
        let client = MockClient {
            response: Ok(serde_json::json!({
                "issues": {"nodes": [{
                    "id": "uuid-123",
                    "identifier": "PROD-587",
                    "title": "Test issue",
                    "state": {"name": "Backlog", "type": "backlog"},
                    "team": {"key": "PROD"},
                }]}
            })),
        };
        let issue = find_issue_by_identifier(&client, "PROD-587").unwrap();
        assert_eq!(issue["id"], "uuid-123");
    }
}
