use chrono::{NaiveDate, Utc};
use serde_json::Value;
use std::collections::HashMap;

/// Formatea una issue en formato compacto: ID [State] labels — Title (age, due)
#[allow(dead_code)]
pub fn format_issue_compact(issue: &Value) -> String {
    let id = issue
        .get("identifier")
        .and_then(|v| v.as_str())
        .unwrap_or("???");
    let title = issue
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(no title)");
    let state_name = issue
        .get("state")
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("Unknown");

    let labels = extract_labels(issue);
    let label_str = if labels.is_empty() {
        String::new()
    } else {
        format!(" {}", labels.join(","))
    };

    let age = format_age(issue);
    let due = format_due(issue);
    let meta = build_meta(&age, &due);

    format!("{id} [{state_name}]{label_str} \u{2014} {title}{meta}")
}

/// Formatea una issue como JSONL
pub fn format_issue_json(issue: &Value) -> String {
    let id = issue
        .get("identifier")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let state_type = issue
        .get("state")
        .and_then(|s| s.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let labels = extract_labels(issue);
    let project = issue
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str());
    let priority = issue.get("priority").and_then(|p| p.as_u64()).unwrap_or(0);
    let due_date = issue.get("dueDate").and_then(|d| d.as_str());
    let age_days = calculate_age_days(issue);
    let overdue = is_overdue(issue);

    let mut obj = serde_json::json!({
        "id": id,
        "state": state_type,
        "labels": labels,
        "title": title,
        "age_days": age_days,
        "priority": priority,
    });

    if let Some(p) = project {
        obj["project"] = serde_json::json!(p);
    }
    if let Some(d) = due_date {
        obj["due"] = serde_json::json!(d);
    }
    obj["overdue"] = serde_json::json!(overdue);

    serde_json::to_string(&obj).unwrap_or_default()
}

/// Footer: ── N issues (X backlog, Y todo, Z in-progress)
pub fn format_footer(issues: &[Value], total: Option<u64>, limit: u32) -> String {
    let count = issues.len();
    let mut state_counts: HashMap<String, usize> = HashMap::new();
    for issue in issues {
        let state = issue
            .get("state")
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("unknown")
            .to_string();
        *state_counts.entry(state).or_insert(0) += 1;
    }

    let mut parts: Vec<String> = Vec::new();
    // Ordenar por count descendente
    let mut counts: Vec<(String, usize)> = state_counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    for (state, n) in &counts {
        let state_lower = state.to_lowercase().replace(' ', "-");
        parts.push(format!("{n} {state_lower}"));
    }

    let breakdown = if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    };

    if let Some(total) = total
        && total > count as u64
    {
        return format!(
            "\u{2500}\u{2500} showing {count} of {total} issues{breakdown} (use --all or --limit N for more)"
        );
    }

    // Si el count == limit, puede haber más
    if count as u32 == limit && limit > 0 {
        return format!(
            "\u{2500}\u{2500} {count} issues{breakdown} (may have more, use --all or --limit N)"
        );
    }

    format!("\u{2500}\u{2500} {count} issues{breakdown}")
}

/// Formato create: ✓ ID created [State] labels — Title
pub fn format_created(issue: &Value) -> String {
    let id = issue
        .get("identifier")
        .and_then(|v| v.as_str())
        .unwrap_or("???");
    let title = issue
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(no title)");
    let state_name = issue
        .get("state")
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("Todo");
    let url = issue.get("url").and_then(|v| v.as_str()).unwrap_or("");

    let labels = extract_labels(issue);
    let label_str = if labels.is_empty() {
        String::new()
    } else {
        format!(" {}", labels.join(","))
    };

    format!("✓ {id} created [{state_name}]{label_str} \u{2014} {title}\n  {url}")
}

/// Formato update: ✓ ID OldState → NewState
pub fn format_updated(id: &str, old_state: &str, new_state: &str) -> String {
    format!("✓ {id} {old_state} → {new_state}")
}

/// Formato view: detalle completo
pub fn format_view(issue: &Value) -> String {
    let id = issue
        .get("identifier")
        .and_then(|v| v.as_str())
        .unwrap_or("???");
    let title = issue
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(no title)");
    let state_name = issue
        .get("state")
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("Unknown");
    let priority = issue.get("priority").and_then(|p| p.as_u64()).unwrap_or(0);
    let priority_str = match priority {
        1 => "P1",
        2 => "P2",
        3 => "P3",
        4 => "P4",
        _ => "P0",
    };

    let labels = extract_labels(issue);
    let label_str = if labels.is_empty() {
        String::new()
    } else {
        format!(" {}", labels.join(","))
    };

    let team = issue
        .get("team")
        .and_then(|t| t.get("key"))
        .and_then(|k| k.as_str())
        .unwrap_or("?");
    let project = issue
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("(none)");

    let age = format_age(issue);
    let due = format_due(issue);

    let mut lines = Vec::new();
    lines.push(format!(
        "{id} [{state_name}] {priority_str}{label_str} \u{2014} {title}"
    ));

    let mut meta_parts = vec![format!("Team: {team}"), format!("Project: {project}")];
    if !age.is_empty() {
        meta_parts.push(format!("Created: {age}"));
    }
    if !due.is_empty() {
        meta_parts.push(format!("Due: {due}"));
    }
    lines.push(format!("  {}", meta_parts.join(" | ")));

    // Descripción
    if let Some(desc) = issue.get("description").and_then(|d| d.as_str())
        && !desc.is_empty()
    {
        lines.push("  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}".to_string());
        for line in desc.lines() {
            lines.push(format!("  {line}"));
        }
        lines.push("  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}".to_string());
    }

    // Relaciones
    let relations = issue
        .get("relations")
        .and_then(|r| r.get("nodes"))
        .and_then(|n| n.as_array());
    if let Some(rels) = relations
        && !rels.is_empty()
    {
        let rel_strs: Vec<String> = rels
            .iter()
            .filter_map(|r| {
                let rel_type = r.get("type")?.as_str()?;
                let related_id = r
                    .get("relatedIssue")
                    .and_then(|i| i.get("identifier"))
                    .and_then(|i| i.as_str())
                    .unwrap_or("?");
                Some(format!("{rel_type} {related_id}"))
            })
            .collect();
        lines.push(format!("  Relations: {}", rel_strs.join(", ")));
    }

    // Comments count
    let comments_count = issue
        .get("comments")
        .and_then(|c| c.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if comments_count > 0 {
        lines.push(format!("  Comments: {comments_count}"));
    }

    lines.join("\n")
}

/// Convierte una issue del API a un objeto plano para TOON (campos uniformes, sin nesting)
pub fn format_issue_toon_obj(issue: &Value) -> Value {
    let id = issue
        .get("identifier")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let state = issue
        .get("state")
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let labels = extract_labels(issue).join(",");
    let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let priority = issue.get("priority").and_then(|p| p.as_u64()).unwrap_or(0);
    let age = format_age(issue);
    let due = format_due(issue);
    let project = issue
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");

    serde_json::json!({
        "id": id,
        "state": state,
        "labels": labels,
        "title": title,
        "priority": priority,
        "age": age,
        "due": due,
        "project": project,
    })
}

/// Formatea un array de issues como TOON
pub fn format_issues_toon(issues: &[&Value]) -> String {
    let toon_issues: Vec<Value> = issues.iter().map(|i| format_issue_toon_obj(i)).collect();
    match toon_format::encode_default(&toon_issues) {
        Ok(toon) => toon,
        Err(e) => format!("Error encoding TOON: {e}"),
    }
}

/// Formatea un epic como JSON
pub fn format_epic_json(epic: &Value) -> String {
    let id = epic.get("slugId").and_then(|v| v.as_str()).unwrap_or("");
    let title = epic.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let status = epic_status_machine(epic);
    let target = epic.get("targetDate").and_then(|v| v.as_str());
    let teams = extract_epic_teams(epic);
    let projects_count = epic
        .get("projects")
        .and_then(|p| p.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|nodes| nodes.len())
        .unwrap_or(0);
    let issues_count = extract_epic_issue_count(epic);

    let mut obj = serde_json::json!({
        "id": id,
        "status": status,
        "title": title,
        "teams": teams,
        "projects": projects_count,
        "issues": issues_count,
    });

    if let Some(target) = target {
        obj["target"] = serde_json::json!(target);
    }

    serde_json::to_string(&obj).unwrap_or_default()
}

pub fn format_epic_created(epic: &Value) -> String {
    let id = epic.get("slugId").and_then(|v| v.as_str()).unwrap_or("???");
    let title = epic
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("(no title)");
    let status = epic_status_human(epic);
    let url = epic.get("url").and_then(|v| v.as_str()).unwrap_or("");

    format!("✓ {id} created [{status}] — {title}\n  {url}")
}

pub fn format_epics_toon(epics: &[&Value]) -> String {
    let toon_epics: Vec<Value> = epics
        .iter()
        .map(|epic| {
            let id = epic.get("slugId").and_then(|v| v.as_str()).unwrap_or("");
            let status = epic_status_machine(epic);
            let title = epic.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let teams = extract_epic_teams(epic).join(",");
            let projects = epic
                .get("projects")
                .and_then(|p| p.get("nodes"))
                .and_then(|n| n.as_array())
                .map(|nodes| nodes.len())
                .unwrap_or(0);
            let target = epic
                .get("targetDate")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            serde_json::json!({
                "id": id,
                "status": status,
                "teams": teams,
                "title": title,
                "projects": projects,
                "target": target,
            })
        })
        .collect();

    match toon_format::encode_default(&toon_epics) {
        Ok(toon) => toon,
        Err(e) => format!("Error encoding TOON: {e}"),
    }
}

pub fn format_epics_footer(epics: &[Value], limit: u32) -> String {
    let count = epics.len();
    let mut status_counts: HashMap<String, usize> = HashMap::new();

    for epic in epics {
        let status = epic_status_machine(epic);
        *status_counts.entry(status).or_insert(0) += 1;
    }

    let mut parts: Vec<String> = status_counts
        .into_iter()
        .map(|(status, count)| format!("{count} {status}"))
        .collect();
    parts.sort();

    let breakdown = if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    };

    if count as u32 == limit && limit > 0 {
        return format!("── {count} epics{breakdown} (may have more, use --all or --limit N)");
    }

    format!("── {count} epics{breakdown}")
}

pub fn format_epic_view(epic: &Value) -> String {
    let id = epic.get("slugId").and_then(|v| v.as_str()).unwrap_or("???");
    let title = epic
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("(no title)");
    let status = epic_status_human(epic);
    let teams = extract_epic_teams(epic);
    let projects = epic
        .get("projects")
        .and_then(|p| p.get("nodes"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    let team_str = if teams.is_empty() {
        "?".to_string()
    } else {
        teams.join(",")
    };
    let mut lines = vec![format!("{id} [{status}] {team_str} — {title}")];

    let mut meta = vec![
        format!("Projects: {}", projects.len()),
        format!("Issues: {}", extract_epic_issue_count(epic)),
    ];
    if !teams.is_empty() {
        meta.push(format!("Teams: {}", teams.join(",")));
    }
    if let Some(target) = epic.get("targetDate").and_then(|v| v.as_str()) {
        meta.push(format!("Target: {target}"));
    }
    lines.push(format!("  {}", meta.join(" | ")));

    if let Some(url) = epic.get("url").and_then(|v| v.as_str()) {
        lines.push(format!("  {url}"));
    }

    if let Some(desc) = epic.get("description").and_then(|d| d.as_str())
        && !desc.is_empty()
    {
        lines.push("  ─────".to_string());
        for line in desc.lines() {
            lines.push(format!("  {line}"));
        }
        lines.push("  ─────".to_string());
    }

    if !projects.is_empty() {
        let project_names: Vec<String> = projects
            .iter()
            .filter_map(|project| {
                let name = project.get("name").and_then(|v| v.as_str())?;
                let teams = project
                    .get("teams")
                    .and_then(|t| t.get("nodes"))
                    .and_then(|n| n.as_array())
                    .map(|nodes| {
                        nodes
                            .iter()
                            .filter_map(|team| {
                                team.get("key")
                                    .and_then(|v| v.as_str())
                                    .map(ToOwned::to_owned)
                            })
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                if teams.is_empty() {
                    Some(name.to_string())
                } else {
                    Some(format!("{name} [{}]", teams.join(",")))
                }
            })
            .collect();
        lines.push(format!("  Projects: {}", project_names.join(", ")));
    }

    let issues = extract_epic_issues(epic);
    if !issues.is_empty() {
        let refs: Vec<&Value> = issues.iter().collect();
        lines.push(format_issues_toon(&refs));
        lines.push(format_footer(&issues, None, 250));
    }

    lines.join("\n")
}

// --- Helpers ---

fn extract_labels(issue: &Value) -> Vec<String> {
    issue
        .get("labels")
        .and_then(|l| l.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|l| {
                    l.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn calculate_age_days(issue: &Value) -> i64 {
    issue
        .get("createdAt")
        .and_then(|c| c.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|created| (Utc::now() - created.to_utc()).num_days())
        .unwrap_or(0)
}

fn format_age(issue: &Value) -> String {
    let days = calculate_age_days(issue);
    if days == 0 {
        "today".to_string()
    } else {
        format!("{days}d")
    }
}

fn is_overdue(issue: &Value) -> bool {
    issue
        .get("dueDate")
        .and_then(|d| d.as_str())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .is_some_and(|due| due < Utc::now().date_naive())
}

fn format_due(issue: &Value) -> String {
    let due_str = match issue.get("dueDate").and_then(|d| d.as_str()) {
        Some(s) => s,
        None => return String::new(),
    };

    match NaiveDate::parse_from_str(due_str, "%Y-%m-%d") {
        Ok(due) => {
            let today = Utc::now().date_naive();
            if due < today {
                "overdue!".to_string()
            } else {
                format!("due:{}", due.format("%b %d"))
            }
        }
        Err(_) => due_str.to_string(),
    }
}

fn extract_epic_teams(epic: &Value) -> Vec<String> {
    let mut teams = Vec::new();
    if let Some(projects) = epic
        .get("projects")
        .and_then(|p| p.get("nodes"))
        .and_then(|n| n.as_array())
    {
        for project in projects {
            if let Some(project_teams) = project
                .get("teams")
                .and_then(|t| t.get("nodes"))
                .and_then(|n| n.as_array())
            {
                for team in project_teams {
                    if let Some(key) = team.get("key").and_then(|v| v.as_str())
                        && !teams.iter().any(|existing| existing == key)
                    {
                        teams.push(key.to_string());
                    }
                }
            }
        }
    }
    teams
}

fn extract_epic_issues(epic: &Value) -> Vec<Value> {
    epic.get("projects")
        .and_then(|p| p.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|projects| {
            let mut issues = Vec::new();
            for project in projects {
                if let Some(nodes) = project
                    .get("issues")
                    .and_then(|i| i.get("nodes"))
                    .and_then(|n| n.as_array())
                {
                    for issue in nodes {
                        issues.push(issue.clone());
                    }
                }
            }
            issues
        })
        .unwrap_or_default()
}

fn extract_epic_issue_count(epic: &Value) -> usize {
    extract_epic_issues(epic).len()
}

fn epic_status_machine(epic: &Value) -> String {
    epic.get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_lowercase()
}

fn epic_status_human(epic: &Value) -> String {
    let status = epic_status_machine(epic);
    let mut chars = status.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => "Unknown".to_string(),
    }
}

#[allow(dead_code)]
fn build_meta(age: &str, due: &str) -> String {
    let mut parts = Vec::new();
    if !age.is_empty() {
        parts.push(age.to_string());
    }
    if !due.is_empty() {
        parts.push(due.to_string());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_issue() -> Value {
        serde_json::json!({
            "identifier": "PROD-587",
            "title": "Importar sesiones desde backup del NAS",
            "state": {"name": "Backlog", "type": "backlog"},
            "labels": {"nodes": [{"name": "qinqin"}]},
            "project": {"name": "Qinqin"},
            "team": {"key": "PROD"},
            "priority": 2,
            "createdAt": "2026-03-11T10:00:00Z",
            "dueDate": "2026-03-11",
            "url": "https://linear.app/frr149/issue/PROD-587"
        })
    }

    // ERR-55: format compacto
    #[test]
    fn test_format_compact_structure() {
        let issue = sample_issue();
        let output = format_issue_compact(&issue);
        assert!(output.starts_with("PROD-587 [Backlog]"));
        assert!(output.contains("qinqin"));
        assert!(output.contains("\u{2014}")); // em-dash
        assert!(output.contains("Importar sesiones"));
    }

    // ERR-59: JSONL válido
    #[test]
    fn test_format_json_valid() {
        let issue = sample_issue();
        let json_str = format_issue_json(&issue);
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["id"], "PROD-587");
        assert_eq!(parsed["state"], "backlog");
        assert_eq!(parsed["labels"][0], "qinqin");
    }

    // ERR-60: sin ANSI
    #[test]
    fn test_format_no_ansi() {
        let issue = sample_issue();
        let output = format_issue_compact(&issue);
        assert!(!output.contains("\x1b["));
        assert!(!output.contains("\x1b("));
    }

    // ERR-56: footer con conteo
    #[test]
    fn test_footer_with_counts() {
        let issues = vec![
            serde_json::json!({"state": {"name": "Backlog"}}),
            serde_json::json!({"state": {"name": "Backlog"}}),
            serde_json::json!({"state": {"name": "Todo"}}),
        ];
        let footer = format_footer(&issues, None, 50);
        assert!(footer.contains("3 issues"));
        assert!(footer.contains("backlog"));
        assert!(footer.contains("todo"));
    }

    // ERR-57: create output
    #[test]
    fn test_format_created() {
        let issue = sample_issue();
        let output = format_created(&issue);
        assert!(output.starts_with("✓ PROD-587 created"));
        assert!(output.contains("[Backlog]"));
        assert!(output.contains("linear.app"));
    }

    // ERR-58: update output
    #[test]
    fn test_format_updated() {
        let output = format_updated("PROD-587", "Backlog", "Done");
        assert_eq!(output, "✓ PROD-587 Backlog → Done");
    }

    #[test]
    fn test_format_epic_created() {
        let epic = serde_json::json!({
            "slugId": "pre-locale",
            "name": "Pre-locale",
            "status": "planned",
            "url": "https://linear.app/frr149/initiative/pre-locale"
        });
        let output = format_epic_created(&epic);
        assert!(output.starts_with("✓ pre-locale created [Planned]"));
        assert!(output.contains("linear.app"));
    }

    #[test]
    fn test_format_epic_json() {
        let epic = serde_json::json!({
            "slugId": "pre-locale",
            "name": "Pre-locale",
            "status": "active",
            "projects": {"nodes": [{"teams": {"nodes": [{"key": "TOOL"}]}}]}
        });
        let output = format_epic_json(&epic);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["id"], "pre-locale");
        assert_eq!(parsed["status"], "active");
        assert_eq!(parsed["teams"][0], "TOOL");
    }
}

#[cfg(test)]
mod toon_tests {
    use super::*;

    #[test]
    fn test_toon_uniform_array() {
        // Con campos uniformes (sin arrays anidados), TOON produce formato tabular
        let issues = serde_json::json!([
            {"id": "PROD-587", "state": "backlog", "labels": "qinqin", "title": "Importar sesiones", "age": "14d", "due": "overdue!"},
            {"id": "PROD-515", "state": "started", "labels": "tokamak", "title": "Fix auth token", "age": "3d", "due": ""},
            {"id": "PROD-529", "state": "backlog", "labels": "wuwei", "title": "Mover media", "age": "4d", "due": "due:Mar 21"}
        ]);
        let toon = toon_format::encode_default(&issues).unwrap();
        eprintln!("\n--- TOON UNIFORM ---\n{toon}\n--- END ---");
        assert!(toon.contains("id"));
        assert!(toon.contains("PROD-587"));
    }

    #[test]
    fn test_toon_from_real_fixture() {
        let path = format!(
            "{}/tests/fixtures/list_tool_5.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let content = std::fs::read_to_string(&path).unwrap();
        let fixture: serde_json::Value = serde_json::from_str(&content).unwrap();
        let issues = fixture["data"]["issues"]["nodes"].as_array().unwrap();

        // Convertir a formato TOON-friendly (campos planos uniformes)
        let toon_issues: Vec<serde_json::Value> =
            issues.iter().map(|i| format_issue_toon_obj(i)).collect();

        let toon = toon_format::encode_default(&toon_issues).unwrap();
        eprintln!("\n--- TOON FROM FIXTURE ---\n{toon}\n--- END ---");
        assert!(!toon.is_empty());
        assert!(toon.contains("TOOL-"));
    }
}
