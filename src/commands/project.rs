use crate::cli::{
    ProjectAction, ProjectCommentOpts, ProjectOpts, ProjectUpdateOpts, ProjectViewOpts,
};
use crate::client::{Client, GraphQLClient};
use crate::commands::comment::{CommentSource, resolve_body_from_source};
use crate::commands::create::get_description_from_args;
use crate::commands::epic::{create_comment, run_project_update, validate_target_date};
use crate::config::Config;
use crate::format;
use serde_json::Value;
use std::io::IsTerminal;

pub fn run(config: &Config, opts: &ProjectOpts) -> Result<(), String> {
    match &opts.action {
        ProjectAction::View(opts) => run_view(config, opts),
        ProjectAction::Update(opts) => run_update(config, opts),
        ProjectAction::Comment(opts) => run_comment(config, opts),
    }
}

fn run_view(config: &Config, opts: &ProjectViewOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;
    let project = find_project_by_ref(&client, &opts.project_ref)?;

    if opts.json {
        println!("{}", serde_json::to_string_pretty(&project).unwrap_or_default());
    } else {
        println!("{}", format::format_project_view(&project));
    }
    Ok(())
}

fn run_update(config: &Config, opts: &ProjectUpdateOpts) -> Result<(), String> {
    if opts.description.is_some() && opts.description_file.is_some() {
        return Err("--description and --description-file are mutually exclusive".to_string());
    }

    let body = get_description_from_args(opts.description.as_ref(), opts.description_file.as_ref())?;

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
        input.insert("description".to_string(), Value::String(summary.to_string()));
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

    let client = Client::new(&config.auth.api_key_ref)?;
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
        println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
    } else {
        println!("{}", format::format_epic_updated(&project_slug, &fields));
    }

    Ok(())
}

fn run_comment(config: &Config, opts: &ProjectCommentOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;

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
