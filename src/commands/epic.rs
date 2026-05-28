use crate::cli::{
    EpicAction, EpicAddOpts, EpicCommentOpts, EpicCreateOpts, EpicListOpts, EpicOpts,
    EpicUpdateOpts, EpicViewOpts,
};
use crate::client::{Client, GraphQLClient, LinearMeta};
use crate::commands::comment::{CommentSource, resolve_body_from_source};
use crate::commands::create::get_description_from_args;
use crate::commands::update::find_issue_by_identifier;
use crate::config::Config;
use crate::format;
use serde_json::Value;
use std::io::IsTerminal;

pub fn run(config: &Config, opts: &EpicOpts) -> Result<(), String> {
    match &opts.action {
        EpicAction::Create(opts) => run_create(config, opts),
        EpicAction::List(opts) => run_list(config, opts),
        EpicAction::View(opts) => run_view(config, opts),
        EpicAction::Add(opts) => run_add(config, opts),
        EpicAction::Update(opts) => run_update(config, opts),
        EpicAction::Comment(opts) => run_comment(config, opts),
    }
}

fn run_create(config: &Config, opts: &EpicCreateOpts) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Could not get cwd: {e}"))?;
    let client = Client::new(&config.auth)?;
    let meta = LinearMeta::fetch(&client)?;

    let team_keys = if let Some(team_keys) = &opts.team {
        team_keys.clone()
    } else {
        vec![config.resolve_team(None, &cwd)?.0]
    };
    let team_ids = resolve_team_ids(&meta, &team_keys)?;

    let body =
        get_description_from_args(opts.description.as_ref(), opts.description_file.as_ref())?;

    let epic = create_epic(&client, &opts.title, body.as_deref(), &team_ids)?;

    if opts.json {
        println!("{}", serde_json::to_string_pretty(&epic).unwrap_or_default());
    } else {
        println!("{}", format::format_epic_created(&epic));
    }

    Ok(())
}

/// Creates an epic atomically: initiative + backing project + link.
///
/// If the backing project cannot be created or linked, every entity created so
/// far is rolled back, so a failed `epic create` never leaves an orphan
/// initiative or project behind (and a retry always starts clean). The returned
/// epic is built entirely from the create mutation's own payload — there is no
/// read-back, which keeps the create path off Linear's GraphQL complexity limit.
fn create_epic(
    client: &dyn GraphQLClient,
    title: &str,
    body: Option<&str>,
    team_ids: &[String],
) -> Result<Value, String> {
    let input = build_initiative_input(title, body);
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

    let mut epic = epic_data
        .get("initiativeCreate")
        .and_then(|c| c.get("initiative"))
        .cloned()
        .ok_or("Could not parse created epic from response")?;
    let epic_id = epic
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Epic has no id")?
        .to_string();
    let epic_slug = epic
        .get("slugId")
        .and_then(|v| v.as_str())
        .unwrap_or(&epic_id)
        .to_string();

    let project = match ensure_backing_project(client, title, body, team_ids, &epic_id) {
        Ok(project) => project,
        Err(err) => {
            // The initiative exists but has no usable backing project. Roll it
            // back so `epic create` is all-or-nothing.
            return Err(match delete_initiative(client, &epic_id) {
                Ok(()) => format!("Epic backing project failed: {err}. Rolled back epic {epic_slug}."),
                Err(rollback_err) => format!(
                    "Epic backing project failed: {err}. \
                     WARNING: could not roll back epic {epic_slug}: {rollback_err}"
                ),
            });
        }
    };

    // Attach the backing project we just created (not a read-back) so `--json`
    // output is complete while the create path stays atomic.
    epic["projects"] = serde_json::json!({ "nodes": [project] });
    Ok(epic)
}

fn run_list(config: &Config, opts: &EpicListOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;
    let mut filter = serde_json::json!({});
    if let Some(team) = opts.team.as_deref() {
        filter["teams"] = serde_json::json!({"some": {"key": {"eq": team.to_uppercase()}}});
    }

    let limit = if opts.all { 250 } else { opts.limit.unwrap_or(50) };
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
    let client = Client::new(&config.auth)?;
    let mut epic = find_epic_by_ref(&client, &opts.epic_id)?;
    attach_epic_issues(&client, &mut epic)?;

    if opts.json {
        println!("{}", serde_json::to_string_pretty(&epic).unwrap_or_default());
    } else {
        println!("{}", format::format_epic_view(&epic));
    }

    Ok(())
}

fn run_add(config: &Config, opts: &EpicAddOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;
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
            ensure_backing_project(&client, epic_name, None, &team_ids, epic_id)?
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
            ))
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
            return Err(format!("Failed to assign {identifier} to epic {epic_slug}."));
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

fn run_update(config: &Config, opts: &EpicUpdateOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;
    let inputs = build_epic_update_inputs(opts)?;

    let epic = find_epic_by_ref(&client, &opts.epic_id)?;
    let epic_id = epic
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Epic has no id")?
        .to_string();
    let epic_slug = epic
        .get("slugId")
        .and_then(|v| v.as_str())
        .unwrap_or(&opts.epic_id)
        .to_string();

    // Per scope decision: do not silently auto-create a backing project. The
    // missing-backing-project case is rare for `lql`-managed epics, and the
    // failure message points to `lql epic add`, which is the supported way.
    let project_id = require_backing_project_id(&epic, &epic_slug)?;

    let updated_initiative =
        run_initiative_update(&client, &epic_id, &inputs.initiative, &epic_slug)?;
    let updated_project = run_project_update(&client, &project_id, &inputs.project)?;

    if opts.json {
        let payload = serde_json::json!({
            "initiative": updated_initiative,
            "project": updated_project,
            "fields": inputs.fields,
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
    } else {
        println!("{}", format::format_epic_updated(&epic_slug, &inputs.fields));
    }

    Ok(())
}

fn run_comment(config: &Config, opts: &EpicCommentOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;

    let is_terminal = std::io::stdin().is_terminal();
    let body = resolve_body_from_source(
        &CommentSource {
            body: opts.body.as_deref(),
            body_flag: opts.body_flag.as_deref(),
            file: opts.file.as_deref(),
            usage_hint: "lql epic comment ID \"text\" or --file or stdin",
        },
        &mut std::io::stdin(),
        is_terminal,
    )?;

    let epic = find_epic_by_ref(&client, &opts.epic_id)?;
    let epic_id = epic
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Epic has no id")?
        .to_string();
    let epic_slug = epic
        .get("slugId")
        .and_then(|v| v.as_str())
        .unwrap_or(&opts.epic_id)
        .to_string();

    let project_id = resolve_single_backing_project_id(&epic, &epic_slug)?;

    create_comment(
        &client,
        serde_json::json!({ "initiativeId": epic_id, "body": body }),
        &format!("epic {epic_slug}"),
    )?;

    if let Some(project_id) = &project_id {
        create_comment(
            &client,
            serde_json::json!({ "projectId": project_id, "body": body }),
            &format!("backing project of {epic_slug}"),
        )?;
        println!("✓ Comment added to {epic_slug} (initiative + backing project)");
    } else {
        println!("✓ Comment added to {epic_slug}");
    }

    Ok(())
}

/// Inputs split per target (initiative + optional backing project) plus the
/// human-readable list of fields actually changed.
#[derive(Debug)]
pub(crate) struct EpicUpdateInputs {
    pub initiative: Value,
    pub project: Value,
    pub fields: Vec<String>,
}

/// Translates `EpicUpdateOpts` into the two `*UpdateInput` payloads.
///
/// Rules enforced here so the runtime can stay focused on the API call:
/// - at least one update flag is required,
/// - `--description` and `--description-file` are mutually exclusive,
/// - the long markdown body goes to `content`, never to the length-capped
///   `description`,
/// - `--summary` updates the short Linear `description`,
/// - `--target-date` is shallow-validated as `YYYY-MM-DD`,
/// - title updates apply the initiative name verbatim and the truncated
///   backing-project name.
pub(crate) fn build_epic_update_inputs(opts: &EpicUpdateOpts) -> Result<EpicUpdateInputs, String> {
    if opts.description.is_some() && opts.description_file.is_some() {
        return Err("--description and --description-file are mutually exclusive".to_string());
    }

    let body = get_description_from_args(opts.description.as_ref(), opts.description_file.as_ref())?;

    let mut initiative = serde_json::Map::new();
    let mut project = serde_json::Map::new();
    let mut fields: Vec<String> = Vec::new();

    if let Some(title) = opts.title.as_deref() {
        initiative.insert("name".to_string(), Value::String(title.to_string()));
        project.insert("name".to_string(), Value::String(project_name_for(title)));
        fields.push("title".to_string());
    }
    if let Some(body) = body {
        initiative.insert("content".to_string(), Value::String(body.clone()));
        project.insert("content".to_string(), Value::String(body));
        fields.push("content".to_string());
    }
    if let Some(summary) = opts.summary.as_deref() {
        initiative.insert("description".to_string(), Value::String(summary.to_string()));
        project.insert("description".to_string(), Value::String(summary.to_string()));
        fields.push("summary".to_string());
    }
    if let Some(target) = opts.target_date.as_deref() {
        validate_target_date(target)?;
        initiative.insert("targetDate".to_string(), Value::String(target.to_string()));
        project.insert("targetDate".to_string(), Value::String(target.to_string()));
        fields.push("targetDate".to_string());
    }

    if fields.is_empty() {
        return Err(
            "No update fields provided. Pass at least one of --title, --description, \
             --description-file, --summary, --target-date."
                .to_string(),
        );
    }

    Ok(EpicUpdateInputs {
        initiative: Value::Object(initiative),
        project: Value::Object(project),
        fields,
    })
}

fn run_initiative_update(
    client: &dyn GraphQLClient,
    epic_uuid: &str,
    input: &Value,
    epic_slug: &str,
) -> Result<Value, String> {
    let data = client.query(
        crate::queries::INITIATIVE_UPDATE_MUTATION,
        serde_json::json!({ "id": epic_uuid, "input": input }),
    )?;
    let success = data
        .get("initiativeUpdate")
        .and_then(|u| u.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if !success {
        return Err(format!("Failed to update epic {epic_slug}."));
    }
    data.get("initiativeUpdate")
        .and_then(|u| u.get("initiative"))
        .cloned()
        .ok_or_else(|| "Could not parse updated initiative from response".to_string())
}

pub(crate) fn run_project_update(
    client: &dyn GraphQLClient,
    project_uuid: &str,
    input: &Value,
) -> Result<Value, String> {
    let data = client.query(
        crate::queries::PROJECT_UPDATE_MUTATION,
        serde_json::json!({ "id": project_uuid, "input": input }),
    )?;
    let success = data
        .get("projectUpdate")
        .and_then(|u| u.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if !success {
        return Err(format!("Failed to update project {project_uuid}."));
    }
    data.get("projectUpdate")
        .and_then(|u| u.get("project"))
        .cloned()
        .ok_or_else(|| "Could not parse updated project from response".to_string())
}

pub(crate) fn create_comment(
    client: &dyn GraphQLClient,
    input: Value,
    target: &str,
) -> Result<(), String> {
    let data = client.query(
        crate::queries::COMMENT_MUTATION,
        serde_json::json!({ "input": input }),
    )?;
    let success = data
        .get("commentCreate")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if success {
        Ok(())
    } else {
        Err(format!("Failed to add comment to {target}"))
    }
}

/// Returns the UUID of the epic's single backing project, or:
/// - `Ok(None)` if the epic has zero backing projects (caller decides what to do),
/// - `Err(...)` if it has more than one.
///
/// MVP rejects the multi-project case loud: `lql` cannot pick the right target
/// on the user's behalf, and silently writing to the first one would be a
/// surprise.
fn resolve_single_backing_project_id(
    epic: &Value,
    epic_slug: &str,
) -> Result<Option<String>, String> {
    let projects = epic
        .get("projects")
        .and_then(|p| p.get("nodes"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    match projects.len() {
        0 => Ok(None),
        1 => Ok(Some(
            projects[0]
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Epic project has no id")?
                .to_string(),
        )),
        n => Err(format!(
            "Epic \"{epic_slug}\" has {n} backing projects. \
             `lql epic update`/`comment` only work when the epic has a single backing project."
        )),
    }
}

/// Returns an error if the epic has no backing project — for callers that
/// require one (update, comment). Suggests `lql epic add` so the user has a
/// clear path forward without `lql` silently creating projects on their
/// behalf.
fn require_backing_project_id(epic: &Value, epic_slug: &str) -> Result<String, String> {
    resolve_single_backing_project_id(epic, epic_slug)?
        .ok_or_else(|| {
            format!(
                "Epic \"{epic_slug}\" has no backing project. \
                 Run `lql epic add {epic_slug} <ISSUE-ID>` to create one, or use `lql epic create`."
            )
        })
}

/// Shallow check: TimelessDate is `YYYY-MM-DD`. Linear will reject impossible
/// dates anyway; we just block obvious typos before the API round-trip.
pub(crate) fn validate_target_date(date: &str) -> Result<(), String> {
    let bytes = date.as_bytes();
    let well_shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, &c)| matches!(i, 4 | 7) || c.is_ascii_digit());
    if !well_shaped {
        return Err(format!(
            "--target-date must be YYYY-MM-DD (got \"{date}\")"
        ));
    }
    Ok(())
}

fn find_epic_by_ref(client: &dyn GraphQLClient, epic_ref: &str) -> Result<Value, String> {
    let normalized = normalize_epic_ref(epic_ref);
    // `slugId` accepts any string, but `id` is validated as a UUID — passing a
    // 12-char slug to `id.eq` is rejected with an "Argument Validation Error".
    // Only offer the `id` branch when the ref actually looks like a UUID.
    let mut or_conditions = vec![serde_json::json!({"slugId": {"eq": normalized}})];
    if looks_like_uuid(&normalized) {
        or_conditions.push(serde_json::json!({"id": {"eq": normalized}}));
    }
    let filter = serde_json::json!({ "or": or_conditions });

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

/// Fetches the issues of every backing project and nests them back under each
/// project node, so the formatter sees the shape it expects.
///
/// `INITIATIVE_BY_REF_QUERY` no longer nests `issues` under each project — that
/// extra connection level multiplied page sizes past Linear's complexity
/// budget. `epic view` fetches them here instead, with a flat, project-filtered
/// `ISSUES_QUERY` that stays comfortably within budget.
fn attach_epic_issues(client: &dyn GraphQLClient, epic: &mut Value) -> Result<(), String> {
    let project_ids: Vec<String> = epic
        .get("projects")
        .and_then(|p| p.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|p| p.get("id").and_then(|v| v.as_str()).map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default();

    if project_ids.is_empty() {
        return Ok(());
    }

    let data = client.query(
        crate::queries::ISSUES_QUERY,
        serde_json::json!({
            "filter": { "project": { "id": { "in": project_ids } } },
            "first": 250,
            "orderBy": "updatedAt",
        }),
    )?;

    let issues = data
        .get("issues")
        .and_then(|i| i.get("nodes"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    if let Some(projects) = epic
        .get_mut("projects")
        .and_then(|p| p.get_mut("nodes"))
        .and_then(|n| n.as_array_mut())
    {
        for project in projects {
            let project_id = project
                .get("id")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned);
            let project_issues: Vec<Value> = issues
                .iter()
                .filter(|issue| {
                    issue
                        .get("project")
                        .and_then(|p| p.get("id"))
                        .and_then(|v| v.as_str())
                        .map(ToOwned::to_owned)
                        == project_id
                })
                .cloned()
                .collect();
            project["issues"] = serde_json::json!({ "nodes": project_issues });
        }
    }

    Ok(())
}

/// Whether a string is shaped like a UUID (`8-4-4-4-12` hex with hyphens).
fn looks_like_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, &c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
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

/// Builds the `InitiativeCreateInput` for a new epic.
///
/// The long markdown body goes in `content`. It must NOT go in `description`:
/// Linear caps `InitiativeCreateInput.description` at ~255 chars and rejects
/// anything longer with an "Argument Validation Error".
fn build_initiative_input(title: &str, body: Option<&str>) -> Value {
    let mut input = serde_json::json!({ "name": title });
    if let Some(body) = body {
        input["content"] = serde_json::json!(body);
    }
    input
}

/// Linear caps `ProjectCreateInput.name` at 80 characters.
const PROJECT_NAME_MAX: usize = 80;

/// The backing project's name, truncated to Linear's 80-char limit.
///
/// `ProjectCreateInput.name` is rejected with an "Argument Validation Error"
/// above 80 chars — and epic titles routinely run longer. The backing project
/// only has to identify the epic, so a truncated title (with an ellipsis to
/// signal it) is fine; the full title lives on the initiative.
fn project_name_for(title: &str) -> String {
    if title.chars().count() <= PROJECT_NAME_MAX {
        return title.to_string();
    }
    let truncated: String = title.chars().take(PROJECT_NAME_MAX - 1).collect();
    format!("{truncated}…")
}

/// Builds the `ProjectCreateInput` for an epic's backing project.
///
/// As with the initiative, the long body belongs in `content`, never in the
/// length-capped `description`. The `name` is truncated to fit Linear's
/// 80-char project-name limit.
fn build_project_input(title: &str, body: Option<&str>, team_ids: &[String]) -> Value {
    let mut input = serde_json::json!({
        "name": project_name_for(title),
        "teamIds": team_ids,
    });
    if let Some(body) = body {
        input["content"] = serde_json::json!(body);
    }
    input
}

fn ensure_backing_project(
    client: &dyn GraphQLClient,
    title: &str,
    body: Option<&str>,
    team_ids: &[String],
    epic_id: &str,
) -> Result<Value, String> {
    let input = build_project_input(title, body, team_ids);
    let project_data = client.query(
        crate::queries::PROJECT_CREATE_MUTATION,
        serde_json::json!({ "input": input }),
    )?;

    let success = project_data
        .get("projectCreate")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if !success {
        return Err("Linear rejected the backing project".to_string());
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

    let link_result = client.query(
        crate::queries::INITIATIVE_TO_PROJECT_CREATE_MUTATION,
        serde_json::json!({
            "input": {
                "initiativeId": epic_id,
                "projectId": project_id,
            }
        }),
    );

    let linked = link_result
        .as_ref()
        .ok()
        .and_then(|d| d.get("initiativeToProjectCreate"))
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if !linked {
        // The project exists but is not linked to the epic. Roll it back so the
        // caller can roll back the initiative and leave nothing behind.
        let reason = match link_result {
            Ok(_) => "could not link the backing project to the epic".to_string(),
            Err(e) => format!("could not link the backing project to the epic: {e}"),
        };
        return Err(match delete_project(client, project_id) {
            Ok(()) => reason,
            Err(rollback_err) => {
                format!("{reason} (and could not roll back project {project_id}: {rollback_err})")
            }
        });
    }

    Ok(project)
}

/// Deletes an initiative — used to roll back a partially-created epic.
fn delete_initiative(client: &dyn GraphQLClient, initiative_id: &str) -> Result<(), String> {
    let data = client.query(
        crate::queries::INITIATIVE_DELETE_MUTATION,
        serde_json::json!({ "id": initiative_id }),
    )?;
    if data
        .get("initiativeDelete")
        .and_then(|d| d.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err("initiativeDelete reported failure".to_string())
    }
}

/// Deletes a project — used to roll back a partially-created epic.
fn delete_project(client: &dyn GraphQLClient, project_id: &str) -> Result<(), String> {
    let data = client.query(
        crate::queries::PROJECT_DELETE_MUTATION,
        serde_json::json!({ "id": project_id }),
    )?;
    if data
        .get("projectDelete")
        .and_then(|d| d.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err("projectDelete reported failure".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queries;
    use std::cell::RefCell;

    #[test]
    fn test_normalize_epic_ref_slug_passthrough() {
        assert_eq!(normalize_epic_ref("pre-locale"), "pre-locale");
    }

    #[test]
    fn test_normalize_epic_ref_from_url() {
        assert_eq!(
            normalize_epic_ref("https://linear.app/example-org/initiative/pre-locale/some-title"),
            "pre-locale"
        );
    }

    #[test]
    fn looks_like_uuid_accepts_uuid_rejects_slug() {
        assert!(looks_like_uuid("bac8bf6d-b199-4a74-91a9-657adeac8ab4"));
        // A Linear initiative slugId is 12 hex chars — not a UUID. Feeding it to
        // an `id` filter triggers "Argument Validation Error".
        assert!(!looks_like_uuid("d9994a56fc60"));
        assert!(!looks_like_uuid("pre-locale"));
        assert!(!looks_like_uuid(""));
        assert!(!looks_like_uuid("bac8bf6d-b199-4a74-91a9-657adeac8abZ"));
    }

    // --- Bug 2: a long body must go to `content`, never the capped `description` ---

    #[test]
    fn build_initiative_input_routes_long_body_to_content() {
        // A body well past the ~255-char `description` cap — the case that
        // crashed `epic create` with "Argument Validation Error".
        let body = "x".repeat(5000);
        let input = build_initiative_input("My epic", Some(&body));
        assert_eq!(input["name"], "My epic");
        assert_eq!(input["content"], serde_json::json!(body));
        assert!(
            input.get("description").is_none(),
            "the long body must not land in the length-capped `description` field"
        );
    }

    #[test]
    fn build_initiative_input_without_body_sets_only_name() {
        let input = build_initiative_input("My epic", None);
        assert_eq!(input["name"], "My epic");
        assert!(input.get("content").is_none());
        assert!(input.get("description").is_none());
    }

    #[test]
    fn build_project_input_routes_long_body_to_content() {
        let body = "y".repeat(5000);
        let team_ids = vec!["team-1".to_string(), "team-2".to_string()];
        let input = build_project_input("My epic", Some(&body), &team_ids);
        assert_eq!(input["name"], "My epic");
        assert_eq!(input["content"], serde_json::json!(body));
        assert_eq!(input["teamIds"], serde_json::json!(team_ids));
        assert!(input.get("description").is_none());
    }

    #[test]
    fn build_project_input_truncates_name_to_linear_limit() {
        // Linear rejects a `ProjectCreateInput.name` over 80 chars — and epic
        // titles routinely run longer.
        let long_title = "X".repeat(200);
        let input = build_project_input(&long_title, None, &[]);
        let name = input["name"].as_str().unwrap();
        assert!(
            name.chars().count() <= 80,
            "project name must fit Linear's 80-char cap, got {}",
            name.chars().count()
        );
        assert!(name.ends_with('…'), "a truncated name should signal it");
    }

    #[test]
    fn build_project_input_keeps_short_name_verbatim() {
        let input = build_project_input("Short epic title", None, &[]);
        assert_eq!(input["name"], "Short epic title");
    }

    #[test]
    fn project_name_truncation_is_char_safe_for_multibyte_titles() {
        // The epic that exposed this bug had a 92-char title full of `—`/`·`.
        let title = "TOK: Setup Lab — inteligencia de configuración en bucle cerrado (audit · measure · optimize)";
        let name = project_name_for(title);
        assert!(name.chars().count() <= 80);
        // Must not panic or split a multi-byte char — round-tripping proves it.
        assert!(name.is_char_boundary(name.len()));
    }

    // --- Bug 3: `epic create` is atomic; a partial failure leaves no orphan ---

    /// Mock client that routes each mutation by its query constant and records
    /// the sequence of mutations it was asked to run.
    struct MockClient {
        calls: RefCell<Vec<&'static str>>,
        initiative_create: Result<Value, String>,
        project_create: Result<Value, String>,
        link: Result<Value, String>,
        initiative_delete: Result<Value, String>,
        project_delete: Result<Value, String>,
    }

    impl Default for MockClient {
        fn default() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                initiative_create: Ok(ok_initiative()),
                project_create: Ok(ok_project()),
                link: Ok(ok_link()),
                initiative_delete: Ok(ok_delete("initiativeDelete")),
                project_delete: Ok(ok_delete("projectDelete")),
            }
        }
    }

    impl GraphQLClient for MockClient {
        fn query(&self, query: &str, _variables: Value) -> Result<Value, String> {
            let (label, response) = if query == queries::INITIATIVE_CREATE_MUTATION {
                ("initiativeCreate", &self.initiative_create)
            } else if query == queries::PROJECT_CREATE_MUTATION {
                ("projectCreate", &self.project_create)
            } else if query == queries::INITIATIVE_TO_PROJECT_CREATE_MUTATION {
                ("initiativeToProjectCreate", &self.link)
            } else if query == queries::INITIATIVE_DELETE_MUTATION {
                ("initiativeDelete", &self.initiative_delete)
            } else if query == queries::PROJECT_DELETE_MUTATION {
                ("projectDelete", &self.project_delete)
            } else {
                panic!("unexpected query in epic create flow");
            };
            self.calls.borrow_mut().push(label);
            response.clone()
        }
    }

    fn ok_initiative() -> Value {
        serde_json::json!({
            "initiativeCreate": {
                "success": true,
                "initiative": {
                    "id": "epic-uuid",
                    "slugId": "epic-slug",
                    "name": "My epic",
                    "status": "Planned",
                    "url": "https://linear.app/example-org/initiative/epic-slug"
                }
            }
        })
    }

    fn ok_project() -> Value {
        serde_json::json!({
            "projectCreate": {
                "success": true,
                "project": { "id": "project-uuid", "name": "My epic" }
            }
        })
    }

    fn ok_link() -> Value {
        serde_json::json!({
            "initiativeToProjectCreate": {
                "success": true,
                "initiativeToProject": { "id": "link-uuid" }
            }
        })
    }

    fn ok_delete(field: &str) -> Value {
        serde_json::json!({ field: { "success": true } })
    }

    #[test]
    fn create_epic_happy_path_attaches_backing_project() {
        let client = MockClient::default();
        let epic = create_epic(&client, "My epic", None, &["team-1".to_string()]).unwrap();
        assert_eq!(epic["slugId"], "epic-slug");
        assert_eq!(epic["projects"]["nodes"][0]["id"], "project-uuid");
        assert_eq!(
            *client.calls.borrow(),
            ["initiativeCreate", "projectCreate", "initiativeToProjectCreate"]
        );
    }

    #[test]
    fn create_epic_rolls_back_initiative_when_project_create_fails() {
        let client = MockClient {
            project_create: Ok(serde_json::json!({ "projectCreate": { "success": false } })),
            ..MockClient::default()
        };
        let err = create_epic(&client, "My epic", None, &["team-1".to_string()]).unwrap_err();
        assert!(err.contains("Rolled back"), "error should report rollback: {err}");
        assert!(
            client.calls.borrow().contains(&"initiativeDelete"),
            "a failed project create must roll back the orphan initiative: {:?}",
            client.calls.borrow()
        );
    }

    #[test]
    fn create_epic_rolls_back_both_when_link_fails() {
        let client = MockClient {
            link: Ok(serde_json::json!({ "initiativeToProjectCreate": { "success": false } })),
            ..MockClient::default()
        };
        let err = create_epic(&client, "My epic", None, &["team-1".to_string()]).unwrap_err();
        assert!(err.contains("Rolled back"), "error should report rollback: {err}");
        let calls = client.calls.borrow();
        assert!(
            calls.contains(&"projectDelete"),
            "a failed link must roll back the project: {calls:?}"
        );
        assert!(
            calls.contains(&"initiativeDelete"),
            "a failed link must roll back the initiative: {calls:?}"
        );
    }

    // ===================================================================
    // `lql epic update` — acceptance tests from
    // docs/epic-update-contract.md.
    // ===================================================================

    fn empty_update_opts(epic_id: &str) -> EpicUpdateOpts {
        EpicUpdateOpts {
            epic_id: epic_id.to_string(),
            title: None,
            description: None,
            description_file: None,
            summary: None,
            target_date: None,
            json: false,
        }
    }

    #[test]
    fn epic_update_requires_at_least_one_flag() {
        let opts = empty_update_opts("pre-locale");
        let err = build_epic_update_inputs(&opts).unwrap_err();
        assert!(
            err.contains("No update fields provided"),
            "should require at least one flag, got: {err}"
        );
    }

    #[test]
    fn epic_update_rejects_description_and_description_file_together() {
        let opts = EpicUpdateOpts {
            description: Some("inline".to_string()),
            description_file: Some("/tmp/plan.md".to_string()),
            ..empty_update_opts("pre-locale")
        };
        let err = build_epic_update_inputs(&opts).unwrap_err();
        assert!(
            err.contains("mutually exclusive"),
            "should reject both body sources, got: {err}"
        );
    }

    #[test]
    fn epic_update_routes_long_body_to_content_not_description() {
        // A body well past Linear's ~255-char `description` cap. The same
        // failure mode as `epic create`: writing it to `description` triggers
        // "Argument Validation Error".
        let body = "z".repeat(5000);
        let opts = EpicUpdateOpts {
            description: Some(body.clone()),
            ..empty_update_opts("pre-locale")
        };
        let inputs = build_epic_update_inputs(&opts).unwrap();
        assert_eq!(inputs.initiative["content"], serde_json::json!(body));
        assert!(inputs.initiative.get("description").is_none());
        assert_eq!(inputs.project["content"], serde_json::json!(body));
        assert!(inputs.project.get("description").is_none());
        assert_eq!(inputs.fields, vec!["content"]);
    }

    #[test]
    fn epic_update_title_applies_to_initiative_and_truncated_project_name() {
        // Linear caps `ProjectCreateInput.name` / `ProjectUpdateInput.name`
        // at 80 chars. Epic titles routinely run longer, so the backing
        // project name must be truncated while the initiative keeps the
        // full title.
        let long_title = "X".repeat(200);
        let opts = EpicUpdateOpts {
            title: Some(long_title.clone()),
            ..empty_update_opts("pre-locale")
        };
        let inputs = build_epic_update_inputs(&opts).unwrap();
        assert_eq!(inputs.initiative["name"], serde_json::json!(long_title));
        let project_name = inputs.project["name"].as_str().unwrap();
        assert!(
            project_name.chars().count() <= 80,
            "backing project name must fit Linear's 80-char cap"
        );
        assert!(project_name.ends_with('…'));
    }

    #[test]
    fn epic_update_summary_targets_short_description_field() {
        let opts = EpicUpdateOpts {
            summary: Some("Short summary".to_string()),
            ..empty_update_opts("pre-locale")
        };
        let inputs = build_epic_update_inputs(&opts).unwrap();
        assert_eq!(inputs.initiative["description"], "Short summary");
        assert_eq!(inputs.project["description"], "Short summary");
        // Long-body `content` must NOT be touched by --summary.
        assert!(inputs.initiative.get("content").is_none());
    }

    #[test]
    fn epic_update_target_date_must_be_iso() {
        let opts = EpicUpdateOpts {
            target_date: Some("tomorrow".to_string()),
            ..empty_update_opts("pre-locale")
        };
        let err = build_epic_update_inputs(&opts).unwrap_err();
        assert!(err.contains("YYYY-MM-DD"), "should reject non-ISO date, got: {err}");
    }

    #[test]
    fn epic_update_target_date_accepts_iso() {
        let opts = EpicUpdateOpts {
            target_date: Some("2026-06-15".to_string()),
            ..empty_update_opts("pre-locale")
        };
        let inputs = build_epic_update_inputs(&opts).unwrap();
        assert_eq!(inputs.initiative["targetDate"], "2026-06-15");
        assert_eq!(inputs.project["targetDate"], "2026-06-15");
    }

    #[test]
    fn require_backing_project_id_zero_projects_returns_hint() {
        let epic = serde_json::json!({"projects": {"nodes": []}});
        let err = require_backing_project_id(&epic, "pre-locale").unwrap_err();
        assert!(
            err.contains("lql epic add"),
            "0-project case must point users to `lql epic add`, got: {err}"
        );
    }

    #[test]
    fn require_backing_project_id_multiple_projects_fails_loud() {
        let epic = serde_json::json!({
            "projects": {"nodes": [
                {"id": "p-1"},
                {"id": "p-2"},
            ]}
        });
        let err = require_backing_project_id(&epic, "pre-locale").unwrap_err();
        assert!(
            err.contains("2 backing projects"),
            ">1 project must fail loud, got: {err}"
        );
    }

    #[test]
    fn require_backing_project_id_single_project_returns_id() {
        let epic = serde_json::json!({
            "projects": {"nodes": [{"id": "project-uuid"}]}
        });
        let id = require_backing_project_id(&epic, "pre-locale").unwrap();
        assert_eq!(id, "project-uuid");
    }
}
