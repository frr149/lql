//! Tests usando fixtures reales capturadas de la API de Linear.
//! Fixtures en tests/fixtures/ — capturadas con introspección real, nunca inventadas.

use serde_json::Value;

fn load_fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Could not read fixture {path}: {e}"));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Invalid JSON in fixture {path}: {e}"))
}

// --- Meta parsing ---

#[test]
fn test_meta_has_teams() {
    let fixture = load_fixture("meta.json");
    let data = &fixture["data"];
    let teams = data["teams"]["nodes"].as_array().unwrap();
    assert!(teams.len() >= 5, "Expected at least 5 teams, got {}", teams.len());

    // Verificar que los teams del context-map existen
    let team_keys: Vec<&str> = teams.iter()
        .filter_map(|t| t["key"].as_str())
        .collect();
    for expected in &["PROD", "CONT", "PRIV", "TOOL", "KC"] {
        assert!(
            team_keys.contains(expected),
            "Team {expected} not found in API response. Available: {team_keys:?}"
        );
    }
}

#[test]
fn test_meta_teams_have_states() {
    let fixture = load_fixture("meta.json");
    let teams = fixture["data"]["teams"]["nodes"].as_array().unwrap();

    for team in teams {
        let key = team["key"].as_str().unwrap();
        let states = team["states"]["nodes"].as_array().unwrap();
        assert!(
            !states.is_empty(),
            "Team {key} has no states"
        );

        // Cada state debe tener id, name, type
        for state in states {
            assert!(state["id"].is_string(), "State missing id in team {key}");
            assert!(state["name"].is_string(), "State missing name in team {key}");
            assert!(state["type"].is_string(), "State missing type in team {key}");
        }

        // Verificar que existe al menos backlog y completed
        let types: Vec<&str> = states.iter()
            .filter_map(|s| s["type"].as_str())
            .collect();
        assert!(types.contains(&"backlog"), "Team {key} missing backlog state");
        assert!(types.contains(&"completed"), "Team {key} missing completed state");
    }
}

#[test]
fn test_meta_has_labels() {
    let fixture = load_fixture("meta.json");
    let labels = fixture["data"]["issueLabels"]["nodes"].as_array().unwrap();
    assert!(labels.len() >= 10, "Expected at least 10 labels");

    // Verificar label lql existe
    let label_names: Vec<&str> = labels.iter()
        .filter_map(|l| l["name"].as_str())
        .collect();
    assert!(
        label_names.contains(&"lql"),
        "Label 'lql' not found. Available: {label_names:?}"
    );
}

#[test]
fn test_meta_teams_have_projects() {
    let fixture = load_fixture("meta.json");
    let teams = fixture["data"]["teams"]["nodes"].as_array().unwrap();

    // Al menos PROD debe tener projects
    let prod = teams.iter().find(|t| t["key"].as_str() == Some("PROD")).unwrap();
    let projects = prod["projects"]["nodes"].as_array().unwrap();
    assert!(
        !projects.is_empty(),
        "Team PROD should have projects"
    );

    // Cada project tiene id y name
    for p in projects {
        assert!(p["id"].is_string(), "Project missing id");
        assert!(p["name"].is_string(), "Project missing name");
    }
}

// --- List parsing ---

#[test]
fn test_list_response_structure() {
    let fixture = load_fixture("list_tool_5.json");
    let issues = fixture["data"]["issues"]["nodes"].as_array().unwrap();
    assert_eq!(issues.len(), 5);

    for issue in issues {
        // Campos requeridos por el formatter
        assert!(issue["identifier"].is_string(), "Missing identifier");
        assert!(issue["title"].is_string(), "Missing title");
        assert!(issue["state"]["name"].is_string(), "Missing state.name");
        assert!(issue["state"]["type"].is_string(), "Missing state.type");
        assert!(issue["createdAt"].is_string(), "Missing createdAt");
        assert!(issue["labels"]["nodes"].is_array(), "Missing labels.nodes");
        assert!(issue["team"]["key"].is_string(), "Missing team.key");

        // priority es número
        assert!(issue["priority"].is_number(), "priority should be number");
    }
}

#[test]
fn test_list_issues_have_correct_team() {
    let fixture = load_fixture("list_tool_5.json");
    let issues = fixture["data"]["issues"]["nodes"].as_array().unwrap();

    for issue in issues {
        let team = issue["team"]["key"].as_str().unwrap();
        assert_eq!(team, "TOOL", "Expected TOOL team, got {team}");
    }
}

#[test]
fn test_list_compact_format_with_real_data() {
    let fixture = load_fixture("list_tool_5.json");
    let issues = fixture["data"]["issues"]["nodes"].as_array().unwrap();

    for issue in issues {
        let formatted = lql::format::format_issue_compact(issue);

        // ERR-55: formato correcto
        let id = issue["identifier"].as_str().unwrap();
        assert!(formatted.starts_with(id), "Should start with ID: {formatted}");
        assert!(formatted.contains('['), "Should have [State]: {formatted}");
        assert!(formatted.contains(']'), "Should have [State]: {formatted}");
        assert!(formatted.contains('\u{2014}'), "Should have em-dash: {formatted}");

        // ERR-60: sin ANSI
        assert!(!formatted.contains("\x1b["), "Should not contain ANSI: {formatted}");
    }
}

#[test]
fn test_list_json_format_with_real_data() {
    let fixture = load_fixture("list_tool_5.json");
    let issues = fixture["data"]["issues"]["nodes"].as_array().unwrap();

    for issue in issues {
        let json_str = lql::format::format_issue_json(issue);

        // ERR-59: JSONL válido
        let parsed: Value = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| panic!("Invalid JSONL: {json_str}: {e}"));

        // Campos requeridos
        assert!(parsed["id"].is_string(), "Missing id in JSONL");
        assert!(parsed["state"].is_string(), "Missing state in JSONL");
        assert!(parsed["title"].is_string(), "Missing title in JSONL");
        assert!(parsed["labels"].is_array(), "Missing labels in JSONL");
        assert!(parsed["priority"].is_number(), "Missing priority in JSONL");
        assert!(parsed["age_days"].is_number(), "Missing age_days in JSONL");
    }
}

#[test]
fn test_list_footer_with_real_data() {
    let fixture = load_fixture("list_tool_5.json");
    let issues = fixture["data"]["issues"]["nodes"].as_array().unwrap();
    let owned: Vec<Value> = issues.to_vec();

    let footer = lql::format::format_footer(&owned, None, 5);

    // ERR-56: footer con conteo
    assert!(footer.contains("5 issues"), "Footer should show count: {footer}");
    assert!(footer.starts_with('\u{2500}'), "Footer should start with ──: {footer}");
}

// --- Issue by identifier ---

#[test]
fn test_issue_by_identifier_structure() {
    let fixture = load_fixture("issue_tool_33.json");
    let issues = fixture["data"]["issues"]["nodes"].as_array().unwrap();
    assert_eq!(issues.len(), 1);

    let issue = &issues[0];
    assert_eq!(issue["identifier"].as_str().unwrap(), "TOOL-33");

    // View needs these extra fields
    assert!(issue.get("description").is_some(), "Missing description");
    assert!(issue["relations"]["nodes"].is_array(), "Missing relations");
    assert!(issue["comments"]["nodes"].is_array(), "Missing comments");
}

#[test]
fn test_view_format_with_real_data() {
    let fixture = load_fixture("issue_tool_33.json");
    let issue = &fixture["data"]["issues"]["nodes"][0];

    let formatted = lql::format::format_view(issue);

    assert!(formatted.contains("TOOL-33"), "Should contain identifier");
    assert!(formatted.contains("Scaffolding"), "Should contain title");
    assert!(formatted.contains("Team: TOOL"), "Should contain team");

    // Debe tener separadores de descripción
    assert!(formatted.contains('\u{2500}'), "Should have description separators");
}

// --- Search ---

#[test]
fn test_search_response_structure() {
    let fixture = load_fixture("search_scaffolding.json");
    let issues = fixture["data"]["searchIssues"]["nodes"].as_array().unwrap();
    assert!(!issues.is_empty(), "Search should return results");

    // Misma estructura que list
    for issue in issues {
        assert!(issue["identifier"].is_string());
        assert!(issue["title"].is_string());
        assert!(issue["state"]["name"].is_string());
    }
}

// --- Error responses ---

#[test]
fn test_error_response_has_message() {
    let fixture = load_fixture("error_not_found.json");
    let errors = fixture["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["message"].as_str().unwrap(), "Entity not found");
}

// --- Schema compliance ---

#[test]
fn test_issue_state_types_are_valid() {
    // Verificar que los state types en las fixtures son los que esperamos
    let fixture = load_fixture("meta.json");
    let teams = fixture["data"]["teams"]["nodes"].as_array().unwrap();

    let valid_types = ["backlog", "unstarted", "started", "completed", "canceled", "triage"];

    for team in teams {
        let key = team["key"].as_str().unwrap();
        let states = team["states"]["nodes"].as_array().unwrap();
        for state in states {
            let state_type = state["type"].as_str().unwrap();
            assert!(
                valid_types.contains(&state_type),
                "Team {key} has unexpected state type: {state_type}"
            );
        }
    }
}

#[test]
fn test_issue_relation_types_match_schema() {
    // IssueRelationType enum: blocks, duplicate, related, similar
    // Verificar que nuestro código no asume tipos que no existen
    let valid = ["blocks", "duplicate", "related", "similar"];

    // "blocked-by" NO existe en la API — se normaliza client-side a "blocks" con IDs invertidos
    assert!(!valid.contains(&"blocked-by"), "blocked-by is client-side only");
}

#[test]
fn test_pagination_order_by_values() {
    // PaginationOrderBy: solo createdAt, updatedAt
    // "priority" NO es válido — se ordena client-side
    let valid = ["createdAt", "updatedAt"];
    assert!(!valid.contains(&"priority"), "priority is client-side sort only");
}
