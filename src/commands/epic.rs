use crate::cli::{EpicAction, EpicAddOpts, EpicCreateOpts, EpicListOpts, EpicOpts, EpicViewOpts};
use crate::client::{Client, GraphQLClient, LinearMeta};
use crate::commands::create::get_description_from_args;
use crate::commands::update::find_issue_by_identifier;
use crate::config::Config;
use crate::format;
use serde_json::Value;

pub fn run(config: &Config, opts: &EpicOpts) -> Result<(), String> {
    match &opts.action {
        EpicAction::Create(opts) => run_create(config, opts),
        EpicAction::List(opts) => run_list(config, opts),
        EpicAction::View(opts) => run_view(config, opts),
        EpicAction::Add(opts) => run_add(config, opts),
    }
}

fn run_create(config: &Config, opts: &EpicCreateOpts) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Could not get cwd: {e}"))?;
    let client = Client::new(&config.auth.api_key_ref)?;
    let meta = LinearMeta::fetch(&client)?;

    let team_keys = if let Some(team_keys) = &opts.team {
        team_keys.clone()
    } else {
        vec![config.resolve_team(None, &cwd)?.0]
    };
    let team_ids = resolve_team_ids(&meta, &team_keys)?;

    let mut input = serde_json::json!({
        "name": opts.title,
    });
    if let Some(description) =
        get_description_from_args(opts.description.as_ref(), opts.description_file.as_ref())?
    {
        input["description"] = serde_json::json!(description);
    }

    let epic_data = client.query(
        crate::queries::INITIATIVE_CREATE_MUTATION,
        serde_json::json!({ "input": input }),
    )?;

    let success = epic_data
        .get("initiativeCreate")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if !success {
        return Err("Failed to create epic.".to_string());
    }

    let epic = epic_data
        .get("initiativeCreate")
        .and_then(|c| c.get("initiative"))
        .cloned()
        .ok_or("Could not parse created epic from response")?;
    let epic_id = epic
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Epic has no id")?;
    let epic_slug = epic
        .get("slugId")
        .and_then(|v| v.as_str())
        .unwrap_or(epic_id);

    // Crear proyecto de backing y enlazarlo al epic.
    // Si falla, reportamos el error pero no intentamos cargar el epic completo
    // (evita una query compleja innecesaria en un epic recién creado).
    let project = match ensure_backing_project(&client, opts.title.as_str(), &team_ids, epic_id) {
        Ok(p) => Some(p),
        Err(err) => {
            eprintln!(
                "Warning: Epic {epic_slug} was created, but its backing project could not be created: {err}"
            );
            None
        }
    };

    // Construir la respuesta a partir de los datos ya disponibles en memoria
    // en lugar de hacer una segunda query compleja (que Linear rechaza con
    // "Query too complex" en epics recién creados).
    let mut epic_response = epic.clone();
    if let Some(proj) = project {
        epic_response["projects"] = serde_json::json!({
            "nodes": [proj]
        });
    } else {
        epic_response["projects"] = serde_json::json!({ "nodes": [] });
    }

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&epic_response).unwrap_or_default()
        );
    } else {
        println!("{}", format::format_epic_created(&epic_response));
    }

    Ok(())
}

fn run_list(config: &Config, opts: &EpicListOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;
    let mut filter = serde_json::json!({});
    if let Some(team) = opts.team.as_deref() {
        filter["teams"] = serde_json::json!({"some": {"key": {"eq": team.to_uppercase()}}});
    }

    let limit = if opts.all {
        250
    } else {
        opts.limit.unwrap_or(50)
    };
    let data = client.query(
        crate::queries::INITIATIVES_QUERY,
        serde_json::json!({
            "filter": filter,
            "first": limit,
            "orderBy": "updatedAt",
        }),
    )?;

    let epics = data
        .get("initiatives")
        .and_then(|i| i.get("nodes"))
        .and_then(|n| n.as_array())
        .ok_or("Could not parse epics from response")?;

    if opts.json {
        for epic in epics {
            println!("{}", format::format_epic_json(epic));
        }
    } else {
        let refs: Vec<&Value> = epics.iter().collect();
        println!("{}", format::format_epics_toon(&refs));
        let owned: Vec<Value> = epics.to_vec();
        println!("{}", format::format_epics_footer(&owned, limit));
    }

    Ok(())
}

fn run_view(config: &Config, opts: &EpicViewOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;
    let epic = find_epic_by_ref(&client, &opts.epic_id)?;

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&epic).unwrap_or_default()
        );
    } else {
        println!("{}", format::format_epic_view(&epic));
    }

    Ok(())
}

fn run_add(config: &Config, opts: &EpicAddOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;
    let meta = LinearMeta::fetch(&client)?;
    let epic = find_epic_by_ref(&client, &opts.epic_id)?;
    let epic_id = epic
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Epic has no id")?;
    let epic_slug = epic
        .get("slugId")
        .and_then(|v| v.as_str())
        .unwrap_or(&opts.epic_id);
    let epic_name = epic.get("name").and_then(|v| v.as_str()).unwrap_or("Epic");

    let mut issues = Vec::new();
    for issue_id in &opts.issue_ids {
        issues.push(find_issue_by_identifier(&client, issue_id)?);
    }

    let project = match epic
        .get("projects")
        .and_then(|p| p.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|nodes| nodes.len())
        .unwrap_or(0)
    {
        0 => {
            let team_ids = resolve_issue_team_ids(&meta, &issues)?;
            ensure_backing_project(&client, epic_name, &team_ids, epic_id)?
        }
        1 => epic
            .get("projects")
            .and_then(|p| p.get("nodes"))
            .and_then(|n| n.as_array())
            .and_then(|nodes| nodes.first())
            .cloned()
            .ok_or("Could not read epic project")?,
        count => {
            return Err(format!(
                "Epic \"{epic_slug}\" has {count} projects. `lql epic add` only works when the epic has a single backing project."
            ));
        }
    };

    let project_id = project
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Epic project has no id")?;
    let project_name = project
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("project");

    let mut updated = Vec::new();
    let mut skipped = Vec::new();
    for issue in &issues {
        let issue_id = issue
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Issue has no id")?;
        let identifier = issue
            .get("identifier")
            .and_then(|v| v.as_str())
            .ok_or("Issue has no identifier")?;
        let current_project_id = issue
            .get("project")
            .and_then(|p| p.get("id"))
            .and_then(|v| v.as_str());

        if current_project_id == Some(project_id) {
            skipped.push(identifier.to_string());
            continue;
        }

        let data = client.query(
            crate::queries::UPDATE_MUTATION,
            serde_json::json!({
                "id": issue_id,
                "input": {"projectId": project_id},
            }),
        )?;
        let success = data
            .get("issueUpdate")
            .and_then(|u| u.get("success"))
            .and_then(|s| s.as_bool())
            .unwrap_or(false);
        if !success {
            return Err(format!(
                "Failed to assign {identifier} to epic {epic_slug}."
            ));
        }
        updated.push(identifier.to_string());
    }

    println!(
        "✓ {epic_slug} assigned {} issues via project {project_name}",
        updated.len()
    );
    if !updated.is_empty() {
        println!("  {}", updated.join(", "));
    }
    if !skipped.is_empty() {
        println!("  Already assigned: {}", skipped.join(", "));
    }

    Ok(())
}

fn find_epic_by_ref(client: &dyn GraphQLClient, epic_ref: &str) -> Result<Value, String> {
    let normalized = normalize_epic_ref(epic_ref);
    // La API de Linear requiere UUID estricto para el campo `id`.
    // Si el ref no es un UUID, usar slugId; si lo es, usar id.
    let filter = if is_uuid(&normalized) {
        serde_json::json!({"id": {"eq": normalized}})
    } else {
        serde_json::json!({"slugId": {"eq": normalized}})
    };

    let data = client.query(
        crate::queries::INITIATIVE_BY_REF_QUERY,
        serde_json::json!({ "filter": filter }),
    )?;

    data.get("initiatives")
        .and_then(|i| i.get("nodes"))
        .and_then(|n| n.as_array())
        .and_then(|nodes| nodes.first())
        .cloned()
        .ok_or_else(|| format!("Epic \"{epic_ref}\" not found."))
}

/// Detecta si un string tiene formato UUID v4 (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)
fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    matches!(
        parts.as_slice(),
        [a, b, c, d, e]
            if a.len() == 8 && b.len() == 4 && c.len() == 4 && d.len() == 4 && e.len() == 12
            && a.chars().all(|ch| ch.is_ascii_hexdigit())
            && b.chars().all(|ch| ch.is_ascii_hexdigit())
            && c.chars().all(|ch| ch.is_ascii_hexdigit())
            && d.chars().all(|ch| ch.is_ascii_hexdigit())
            && e.chars().all(|ch| ch.is_ascii_hexdigit())
    )
}

fn normalize_epic_ref(epic_ref: &str) -> String {
    let trimmed = epic_ref.trim().trim_end_matches('/');
    if let Some((_, rest)) = trimmed.split_once("/initiative/") {
        return rest.split('/').next().unwrap_or(rest).to_string();
    }
    trimmed.to_string()
}

fn resolve_team_ids(meta: &LinearMeta, team_keys: &[String]) -> Result<Vec<String>, String> {
    let mut team_ids = Vec::new();
    for key in team_keys {
        let team = meta.find_team(key)?;
        if !team_ids.iter().any(|id| id == &team.id) {
            team_ids.push(team.id.clone());
        }
    }
    Ok(team_ids)
}

fn resolve_issue_team_ids(meta: &LinearMeta, issues: &[Value]) -> Result<Vec<String>, String> {
    let team_keys: Vec<String> = issues
        .iter()
        .filter_map(|issue| {
            issue
                .get("team")
                .and_then(|t| t.get("key"))
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
        })
        .collect();
    resolve_team_ids(meta, &team_keys)
}

fn ensure_backing_project(
    client: &dyn GraphQLClient,
    title: &str,
    team_ids: &[String],
    epic_id: &str,
) -> Result<Value, String> {
    let project_data = client.query(
        crate::queries::PROJECT_CREATE_MUTATION,
        serde_json::json!({
            "input": {
                "name": title,
                "teamIds": team_ids,
            }
        }),
    )?;

    let success = project_data
        .get("projectCreate")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if !success {
        return Err("Failed to create backing project.".to_string());
    }

    let project = project_data
        .get("projectCreate")
        .and_then(|c| c.get("project"))
        .cloned()
        .ok_or("Could not parse created project from response")?;
    let project_id = project
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Backing project has no id")?;

    let link_data = client.query(
        crate::queries::INITIATIVE_TO_PROJECT_CREATE_MUTATION,
        serde_json::json!({
            "input": {
                "initiativeId": epic_id,
                "projectId": project_id,
            }
        }),
    )?;

    let linked = link_data
        .get("initiativeToProjectCreate")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if !linked {
        return Err("Failed to link backing project to epic.".to_string());
    }

    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_epic_ref_slug_passthrough() {
        assert_eq!(normalize_epic_ref("pre-locale"), "pre-locale");
    }

    #[test]
    fn test_normalize_epic_ref_from_url() {
        assert_eq!(
            normalize_epic_ref("https://linear.app/frr149/initiative/pre-locale/some-title"),
            "pre-locale"
        );
    }

    #[test]
    fn test_is_uuid_valid() {
        assert!(is_uuid("6c9e5137-c499-4518-920b-41dcaa779a27"));
        assert!(is_uuid("d7262dfc-6061-498a-8d1b-438e6d2b93b5"));
    }

    #[test]
    fn test_is_uuid_slug_rejected() {
        assert!(!is_uuid("pre-locale"));
        assert!(!is_uuid("3fdf346ca813"));
        assert!(!is_uuid("v10-bastidor-3fdf346ca813"));
    }

    #[test]
    fn test_is_uuid_short_rejected() {
        assert!(!is_uuid("6c9e5137-c499-4518-920b"));
        assert!(!is_uuid(""));
    }
}
