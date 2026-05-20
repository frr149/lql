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
    let client = Client::new(&config.auth.api_key_ref)?;
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
    let client = Client::new(&config.auth.api_key_ref)?;
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

/// Builds the `ProjectCreateInput` for an epic's backing project.
///
/// As with the initiative, the long body belongs in `content`, never in the
/// length-capped `description`.
fn build_project_input(title: &str, body: Option<&str>, team_ids: &[String]) -> Value {
    let mut input = serde_json::json!({
        "name": title,
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
}
