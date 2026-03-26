use crate::cli::{self, SearchOpts};
use crate::client::{Client, GraphQLClient};
use crate::config::Config;
use crate::format;

pub fn run(config: &Config, opts: &SearchOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;

    let mut filter = serde_json::json!({});

    if let Some(ref team) = opts.team {
        // Comprobar teams retirados
        if let Some(msg) = config.retired_teams.get(team.as_str()) {
            return Err(format!("Team {team} is retired. {msg}"));
        }
        filter["team"] = serde_json::json!({"key": {"eq": team}});
    }

    if let Some(ref states) = opts.state {
        let state_types: Vec<String> = states
            .iter()
            .map(|s| cli::normalize_state(s, &config.state_aliases))
            .collect();
        let state_filter: Vec<serde_json::Value> = state_types
            .iter()
            .map(|t| serde_json::json!({"eq": *t}))
            .collect();
        filter["state"] = serde_json::json!({"type": {"or": state_filter}});
    }

    let limit = opts.limit.unwrap_or(config.defaults.limit);

    let variables = serde_json::json!({
        "term": opts.query,
        "filter": filter,
        "first": limit,
    });

    let data = client.query(crate::queries::SEARCH_QUERY, variables)?;

    let issues = data
        .get("searchIssues")
        .and_then(|s| s.get("nodes"))
        .and_then(|n| n.as_array())
        .ok_or("Could not parse search results")?;

    if opts.json {
        for issue in issues {
            println!("{}", format::format_issue_json(issue));
        }
    } else {
        let issue_refs: Vec<&serde_json::Value> = issues.iter().collect();
        println!("{}", format::format_issues_toon(&issue_refs));
        println!("{}", format::format_footer(issues, None, limit));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::format;

    // ERR-64: search sin resultados muestra "0 issues", no error
    #[test]
    fn test_search_empty_results_footer() {
        let empty: Vec<serde_json::Value> = vec![];
        let footer = format::format_footer(&empty, None, 50);
        assert!(footer.contains("0 issues"), "{footer}");
    }

    // ERR-64b: search con resultados formatea correctamente
    #[test]
    fn test_search_results_toon() {
        let issues = vec![serde_json::json!({
            "identifier": "PROD-587",
            "state": {"name": "Backlog", "type": "backlog"},
            "labels": {"nodes": [{"name": "tokamak"}]},
            "title": "Test issue",
            "priority": 2,
            "createdAt": "2026-03-11T10:00:00Z",
            "dueDate": null,
            "project": {"name": "Tokamak"},
            "team": {"key": "PROD"},
        })];
        let refs: Vec<&serde_json::Value> = issues.iter().collect();
        let toon = format::format_issues_toon(&refs);
        assert!(toon.contains("PROD-587"), "{toon}");
    }
}
