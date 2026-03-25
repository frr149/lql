use crate::cli::{self, ListOpts};
use crate::client::{Client, LinearMeta};
use crate::config::Config;
use crate::format;

pub fn run(config: &Config, opts: &ListOpts) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Could not get cwd: {e}"))?;
    let client = Client::new(&config.auth.api_key_ref)?;
    let meta = LinearMeta::fetch(&client)?;

    // Resolver team
    let mut filter = serde_json::json!({});

    if !opts.all_teams {
        let (team_key, _ctx_project, ctx_label) = if let Some(team) = opts.team.as_deref() {
            config.resolve_team(Some(team), &cwd)?
        } else {
            config.resolve_team(None, &cwd)?
        };

        let team_info = meta.find_team(&team_key)?;
        filter["team"] = serde_json::json!({"key": {"eq": team_key}});

        // Resolver labels: explicit > context-map
        let label_names = opts.label.as_ref().map(|l| l.as_slice()).or_else(|| {
            ctx_label.as_ref().map(|l| std::slice::from_ref(l))
        });
        if let Some(names) = label_names {
            if opts.label.is_some() {
                // Solo validar labels explícitos
                let mut label_ids = Vec::new();
                for name in names {
                    let label = meta.find_label(name)?;
                    label_ids.push(serde_json::json!({"id": {"eq": label.id}}));
                }
                if label_ids.len() == 1 {
                    filter["labels"] = serde_json::json!({"some": label_ids[0]});
                } else {
                    filter["labels"] = serde_json::json!({"some": {"or": label_ids}});
                }
            }
            // No filtrar por label del context-map en list (mostraría solo ese label)
        }

        // Resolver project
        if let Some(project_name) = opts.project.as_deref() {
            let project = meta.find_project(team_info, project_name)?;
            filter["project"] = serde_json::json!({"id": {"eq": project.id}});
        }

        // Resolver states
        let state_types: Vec<String> = if let Some(ref states) = opts.state {
            states
                .iter()
                .map(|s| cli::normalize_state(s, &config.state_aliases))
                .collect()
        } else {
            config.defaults.states.clone()
        };
        let matching_states = meta.find_state_by_type_list(team_info, &state_types);
        if !matching_states.is_empty() {
            let state_ids: Vec<serde_json::Value> = matching_states
                .iter()
                .map(|s| serde_json::json!({"eq": s.id}))
                .collect();
            filter["state"] = serde_json::json!({"id": {"or": state_ids}});
        }
    } else {
        // all-teams: filtrar por estados por defecto
        let state_types: Vec<String> = if let Some(ref states) = opts.state {
            states
                .iter()
                .map(|s| cli::normalize_state(s, &config.state_aliases))
                .collect()
        } else {
            config.defaults.states.clone()
        };
        let state_type_filter: Vec<serde_json::Value> = state_types
            .iter()
            .map(|t| serde_json::json!({"eq": *t}))
            .collect();
        filter["state"] = serde_json::json!({"type": {"or": state_type_filter}});

        if let Some(ref names) = opts.label {
            let mut label_ids = Vec::new();
            for name in names {
                let label = meta.find_label(name)?;
                label_ids.push(serde_json::json!({"id": {"eq": label.id}}));
            }
            if label_ids.len() == 1 {
                filter["labels"] = serde_json::json!({"some": label_ids[0]});
            } else {
                filter["labels"] = serde_json::json!({"some": {"or": label_ids}});
            }
        }
    }

    // Prioridad
    if let Some(ref prio) = opts.priority {
        let p = cli::normalize_priority(prio, &config.priority_aliases)?;
        filter["priority"] = serde_json::json!({"eq": p});
    }

    // Overdue
    if opts.overdue {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        filter["dueDate"] = serde_json::json!({"lt": today});
    }

    let limit = if opts.all {
        250 // Linear max
    } else {
        opts.limit.unwrap_or(config.defaults.limit)
    };

    let sort = cli::normalize_sort(&opts.sort);
    let order_by = match sort.as_str() {
        "updatedAt" => "updatedAt",
        "createdAt" => "createdAt",
        _ => "priority",
    };

    let variables = serde_json::json!({
        "filter": filter,
        "first": limit,
        "orderBy": order_by,
    });

    let data = client.query(crate::queries::ISSUES_QUERY, variables)?;

    let issues = data
        .get("issues")
        .and_then(|i| i.get("nodes"))
        .and_then(|n| n.as_array())
        .ok_or("Could not parse issues from response")?;

    if opts.json {
        for issue in issues {
            println!("{}", format::format_issue_json(issue));
        }
    } else {
        for issue in issues {
            println!("{}", format::format_issue_compact(issue));
        }
        println!("{}", format::format_footer(issues, None, limit));
    }

    Ok(())
}
