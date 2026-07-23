use crate::cli::{
    ProjectAction, ProjectCommentOpts, ProjectCreateOpts, ProjectOpts, ProjectUpdateOpts,
    ProjectViewOpts,
};
use crate::client::{Client, GraphQLClient, LinearMeta};
use crate::commands::comment::{CommentSource, resolve_body_from_source};
use crate::commands::create::{get_description_from_args, reject_conflicting_description_sources};
use crate::commands::epic::{
    PROJECT_NAME_MAX, create_comment, run_project_update, validate_target_date,
};
use crate::config::{Config, TeamSource};
use crate::format;
use serde_json::Value;
use std::io::IsTerminal;

pub fn run(config: &Config, opts: &ProjectOpts) -> Result<(), String> {
    match &opts.action {
        ProjectAction::Create(opts) => run_create(config, opts),
        ProjectAction::View(opts) => run_view(config, opts),
        ProjectAction::Update(opts) => run_update(config, opts),
        ProjectAction::Comment(opts) => run_comment(config, opts),
    }
}

/// Builds the `ProjectCreateInput` for `lql project create`. Pure and fallible:
/// rejects an empty name and a name over Linear's 80-char cap. Unlike an epic's
/// auto-managed backing project (which truncates), an interactive create fails
/// loud rather than silently shortening the user's name.
fn build_project_create_input(
    name: &str,
    body: Option<&str>,
    team_id: &str,
) -> Result<Value, String> {
    check_project_name(name)?;
    let mut input = serde_json::json!({
        "name": name.trim(),
        "teamIds": [team_id],
    });
    if let Some(body) = body {
        input["content"] = serde_json::json!(body);
    }
    Ok(input)
}

/// Validates the project name against Linear's rules. Pure, so `run_create` can
/// call it *before* any network request (fail loud without a wasted round-trip).
fn check_project_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Project name is empty".to_string());
    }
    let len = trimmed.chars().count();
    if len > PROJECT_NAME_MAX {
        return Err(format!(
            "Project name is {len} chars; Linear caps project names at {PROJECT_NAME_MAX}. \
             Shorten it."
        ));
    }
    Ok(())
}

/// Extracts the created project node from the mutation response, failing loud
/// when the server reports `success: false` or omits the node. Success is read
/// from the server's answer, never assumed from the request (semantic honesty).
fn project_from_create_response(data: &Value) -> Result<Value, String> {
    let success = data
        .get("projectCreate")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if !success {
        return Err("Linear rejected the project creation".to_string());
    }
    // A created project is proven by a real `id` in the response node, not by
    // `success` alone: `null`, `{}`, `false` or `[]` must all fail rather than
    // print a hollow "created (unknown)". Read identity from the server's answer.
    let node = data
        .get("projectCreate")
        .and_then(|c| c.get("project"))
        .ok_or("Could not parse created project from response")?;
    let has_id = node
        .get("id")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty());
    if !has_id {
        return Err("Project creation response has no project id".to_string());
    }
    Ok(node.clone())
}

fn run_create(config: &Config, opts: &ProjectCreateOpts) -> Result<(), String> {
    reject_conflicting_description_sources(
        opts.description.as_ref(),
        opts.description_file.as_ref(),
    )?;
    // Validate the name up front so a bad name fails before any network request.
    check_project_name(&opts.name)?;

    let cwd = std::env::current_dir().map_err(|e| format!("Could not get cwd: {e}"))?;
    let (team_key, _project, _label, team_source) =
        config.resolve_team(opts.team.as_deref(), &cwd)?;
    if team_source == TeamSource::Default {
        crate::print_warning(
            &crate::config::team_fallback_warning(&team_key),
            crate::cli::machine_mode(),
        );
    }

    let client = Client::new(&config.auth)?;
    let meta = LinearMeta::fetch(&client)?;
    let team = meta.find_team(&team_key)?;

    let body =
        get_description_from_args(opts.description.as_ref(), opts.description_file.as_ref())?;
    let input = build_project_create_input(&opts.name, body.as_deref(), &team.id)?;

    let data = client.query(
        crate::queries::PROJECT_CREATE_MUTATION,
        serde_json::json!({ "input": input }),
    )?;
    let created = project_from_create_response(&data)?;

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&created).unwrap_or_default()
        );
    } else {
        // Name/url come from the server's node, not from opts.name.
        let name = created
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)");
        let url = created.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if url.is_empty() {
            println!("\u{2713} Project created: {name}");
        } else {
            println!("\u{2713} Project created: {name} \u{2014} {url}");
        }
    }
    Ok(())
}

fn run_view(config: &Config, opts: &ProjectViewOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;
    let project = find_project_by_ref(&client, &opts.project_ref)?;

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&project).unwrap_or_default()
        );
    } else {
        println!("{}", format::format_project_view(&project));
    }
    Ok(())
}

fn run_update(config: &Config, opts: &ProjectUpdateOpts) -> Result<(), String> {
    reject_conflicting_description_sources(
        opts.description.as_ref(),
        opts.description_file.as_ref(),
    )?;

    let body =
        get_description_from_args(opts.description.as_ref(), opts.description_file.as_ref())?;

    let mut input = serde_json::Map::new();
    let mut fields: Vec<String> = Vec::new();

    if let Some(title) = opts.title.as_deref() {
        if title.chars().count() > 80 {
            return Err(format!(
                "Project --title is {} chars; Linear caps project names at 80. \
                 Shorten it or use `lql epic update` (which truncates the backing project name automatically).",
                title.chars().count()
            ));
        }
        input.insert("name".to_string(), Value::String(title.to_string()));
        fields.push("title".to_string());
    }
    if let Some(body) = body {
        input.insert("content".to_string(), Value::String(body));
        fields.push("content".to_string());
    }
    if let Some(summary) = opts.summary.as_deref() {
        input.insert(
            "description".to_string(),
            Value::String(summary.to_string()),
        );
        fields.push("summary".to_string());
    }
    if let Some(target) = opts.target_date.as_deref() {
        validate_target_date(target)?;
        input.insert("targetDate".to_string(), Value::String(target.to_string()));
        fields.push("targetDate".to_string());
    }

    if fields.is_empty() {
        return Err(
            "No update fields provided. Pass at least one of --title, --description, \
             --description-file, --summary, --target-date."
                .to_string(),
        );
    }

    let client = Client::new(&config.auth)?;
    let project = find_project_by_ref(&client, &opts.project_ref)?;
    let project_id = project
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Project has no id")?
        .to_string();
    let project_slug = project
        .get("slugId")
        .and_then(|v| v.as_str())
        .unwrap_or(&opts.project_ref)
        .to_string();

    let updated = run_project_update(&client, &project_id, &Value::Object(input))?;

    if opts.json {
        let payload = serde_json::json!({
            "project": updated,
            "fields": fields,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_default()
        );
    } else {
        println!("{}", format::format_epic_updated(&project_slug, &fields));
    }

    Ok(())
}

fn run_comment(config: &Config, opts: &ProjectCommentOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;

    let is_terminal = std::io::stdin().is_terminal();
    let body = resolve_body_from_source(
        &CommentSource {
            body: opts.body.as_deref(),
            body_flag: opts.body_flag.as_deref(),
            file: opts.file.as_deref(),
            usage_hint: "lql project comment ID \"text\" or --file or stdin",
        },
        &mut std::io::stdin(),
        is_terminal,
    )?;

    let project = find_project_by_ref(&client, &opts.project_ref)?;
    let project_id = project
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Project has no id")?
        .to_string();
    let project_slug = project
        .get("slugId")
        .and_then(|v| v.as_str())
        .unwrap_or(&opts.project_ref)
        .to_string();

    create_comment(
        &client,
        serde_json::json!({ "projectId": project_id, "body": body }),
        &format!("project {project_slug}"),
    )?;
    println!("✓ Comment added to project {project_slug}");

    Ok(())
}

/// Resolves a project by UUID, slugId, or name (case-insensitive name match).
///
/// `id.eq` is rejected for non-UUID strings (validation error), so we only
/// include the `id` branch when the ref actually looks like a UUID. Name
/// matches use `nameIgnoreCase.eq` so `lql project view "Bastidor v1.0"`
/// works without the user knowing the slug.
pub(crate) fn find_project_by_ref(
    client: &dyn GraphQLClient,
    project_ref: &str,
) -> Result<Value, String> {
    let trimmed = project_ref.trim().trim_end_matches('/').to_string();
    let mut or_conditions = vec![
        serde_json::json!({"slugId": {"eq": trimmed}}),
        serde_json::json!({"name": {"eqIgnoreCase": trimmed}}),
    ];
    if looks_like_uuid(&trimmed) {
        or_conditions.push(serde_json::json!({"id": {"eq": trimmed}}));
    }
    let filter = serde_json::json!({ "or": or_conditions });

    let data = client.query(
        crate::queries::PROJECT_BY_REF_QUERY,
        serde_json::json!({ "filter": filter }),
    )?;

    data.get("projects")
        .and_then(|p| p.get("nodes"))
        .and_then(|n| n.as_array())
        .and_then(|nodes| nodes.first())
        .cloned()
        .ok_or_else(|| format!("Project \"{project_ref}\" not found."))
}

fn looks_like_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, &c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // T02: the input carries the name and the resolved team id, nothing else.
    #[test]
    fn test_project_create_input_has_name_and_team() {
        let input = build_project_create_input("Web arc", None, "team-uuid").unwrap();
        assert_eq!(input["name"], "Web arc");
        assert_eq!(input["teamIds"], serde_json::json!(["team-uuid"]));
        assert!(input.get("content").is_none()); // no body → no content key
    }

    // T02: a body is placed in `content`, never the length-capped `description`.
    #[test]
    fn test_project_create_input_body_goes_to_content() {
        let input = build_project_create_input("Name", Some("# Long body"), "t").unwrap();
        assert_eq!(input["content"], "# Long body");
        assert!(input.get("description").is_none());
    }

    // T02: empty / whitespace-only name is a typed error before any request.
    #[test]
    fn test_project_create_rejects_empty_name() {
        assert!(build_project_create_input("   ", None, "t").is_err());
    }

    // T02: a name over Linear's 80-char cap is a typed error (fail loud, not
    // silently truncate the way an epic's backing project does).
    #[test]
    fn test_project_create_rejects_overlong_name() {
        let long = "x".repeat(PROJECT_NAME_MAX + 1);
        let err = build_project_create_input(&long, None, "t").unwrap_err();
        assert!(err.contains("caps project names at 80"), "{err}");
    }

    // T02: passing both description sources is rejected (no silent drop).
    #[test]
    fn test_project_create_description_flags_mutually_exclusive() {
        let d = "inline".to_string();
        let f = "file.md".to_string();
        assert!(reject_conflicting_description_sources(Some(&d), Some(&f)).is_err());
    }

    // T02 [Sheldon #5]: success/name are read from the SERVER response node, not
    // from the request. A server node with a different name wins.
    #[test]
    fn test_project_create_reads_name_from_response() {
        let data = serde_json::json!({
            "projectCreate": {
                "success": true,
                "project": { "id": "p1", "name": "Server-Chosen Name", "url": "https://x" }
            }
        });
        let node = project_from_create_response(&data).unwrap();
        assert_eq!(node["name"], "Server-Chosen Name");
    }

    // T02 [Sheldon #5]: success:false is an error, never exit 0.
    #[test]
    fn test_project_create_success_false_is_error() {
        let data = serde_json::json!({ "projectCreate": { "success": false } });
        assert!(project_from_create_response(&data).is_err());
    }

    // T02 [Sheldon #5]: success:true but a missing project node is an error.
    #[test]
    fn test_project_create_missing_node_is_error() {
        let data = serde_json::json!({ "projectCreate": { "success": true } });
        assert!(project_from_create_response(&data).is_err());
    }

    // T02 [Sheldon verify]: success:true with a null / empty / wrong-typed node
    // (no real id) must all be errors, not a hollow "created (unknown)".
    #[test]
    fn test_project_create_node_without_id_is_error() {
        for node in [
            serde_json::json!(null),
            serde_json::json!({}),
            serde_json::json!(false),
            serde_json::json!([]),
            serde_json::json!({ "id": "" }),
        ] {
            let data = serde_json::json!({ "projectCreate": { "success": true, "project": node } });
            assert!(
                project_from_create_response(&data).is_err(),
                "node {node} should be rejected"
            );
        }
    }

    // T02: a node with a real id is accepted and returned verbatim.
    #[test]
    fn test_project_create_valid_node_is_ok() {
        let data = serde_json::json!({
            "projectCreate": { "success": true, "project": { "id": "p1", "name": "X" } }
        });
        let node = project_from_create_response(&data).unwrap();
        assert_eq!(node["id"], "p1");
    }
}
