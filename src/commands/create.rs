use crate::cli::{self, CreateOpts};
use crate::client::{Client, LinearMeta};
use crate::config::Config;
use crate::format;
use std::io::{IsTerminal, Read};

pub fn run(config: &Config, opts: &CreateOpts) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Could not get cwd: {e}"))?;
    let client = Client::new(&config.auth.api_key_ref)?;
    let meta = LinearMeta::fetch(&client)?;

    // Resolver team/project/label
    let (team_key, ctx_project, ctx_label) =
        config.resolve_team(opts.team.as_deref(), &cwd)?;
    let team = meta.find_team(&team_key)?;

    // Construir input de la mutación
    let mut input = serde_json::json!({
        "title": opts.title,
        "teamId": team.id,
    });

    // Descripción: inline > fichero > stdin
    let description = get_description(opts)?;
    if let Some(desc) = description {
        input["description"] = serde_json::json!(desc);
    }

    // Estado
    if let Some(ref state_str) = opts.state {
        let state_type = cli::normalize_state(state_str, &config.state_aliases);
        if let Some(state) = meta.find_state(team, &state_type) {
            input["stateId"] = serde_json::json!(state.id);
        }
    }

    // Prioridad
    if let Some(ref prio) = opts.priority {
        let p = cli::normalize_priority(prio, &config.priority_aliases)?;
        input["priority"] = serde_json::json!(p);
    }

    // Proyecto: explicit > context-map
    let project_name = opts.project.as_deref().or(ctx_project.as_deref());
    if let Some(name) = project_name {
        let project = meta.find_project(team, name)?;
        input["projectId"] = serde_json::json!(project.id);
    }

    // Labels: explicit > context-map
    let label_names: Vec<String> = if let Some(ref labels) = opts.label {
        labels.clone()
    } else if let Some(ref ctx_l) = ctx_label {
        vec![ctx_l.clone()]
    } else {
        vec![]
    };

    if !label_names.is_empty() {
        let mut label_ids = Vec::new();
        for name in &label_names {
            let label = meta.find_label(name)?;
            label_ids.push(serde_json::json!(label.id));
        }
        input["labelIds"] = serde_json::json!(label_ids);
    }

    // Due date
    if let Some(ref due) = opts.due {
        let date = parse_due_date(due)?;
        input["dueDate"] = serde_json::json!(date);
    }

    // Detección de duplicados
    if !opts.force {
        check_duplicates(&client, &opts.title)?;
    }

    // Crear
    let variables = serde_json::json!({"input": input});
    let data = client.query(crate::queries::CREATE_MUTATION, variables)?;

    let issue = data
        .get("issueCreate")
        .and_then(|c| c.get("issue"))
        .ok_or("Could not parse created issue from response")?;

    if opts.json {
        println!("{}", format::format_issue_json(issue));
    } else {
        println!("{}", format::format_created(issue));
    }

    Ok(())
}

fn get_description(opts: &CreateOpts) -> Result<Option<String>, String> {
    // Inline
    if let Some(ref desc) = opts.description {
        return Ok(Some(desc.clone()));
    }

    // Fichero
    if let Some(ref path) = opts.description_file {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Could not read description file {path}: {e}"))?;
        return Ok(Some(content));
    }

    // stdin (solo si no es TTY)
    if !atty_stdin() {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("Could not read from stdin: {e}"))?;
        if !buf.is_empty() {
            return Ok(Some(buf));
        }
    }

    Ok(None)
}

/// Comprueba si stdin es un TTY
fn atty_stdin() -> bool {
    std::io::stdin().is_terminal()
}

fn check_duplicates(client: &Client, title: &str) -> Result<(), String> {
    let variables = serde_json::json!({
        "term": title,
        "first": 5,
    });

    if let Ok(data) = client.query(crate::queries::SEARCH_QUERY, variables) {
        if let Some(nodes) = data
            .get("searchIssues")
            .and_then(|s| s.get("nodes"))
            .and_then(|n| n.as_array())
        {
            let similar: Vec<String> = nodes
                .iter()
                .filter_map(|n| {
                    let id = n.get("identifier")?.as_str()?;
                    let t = n.get("title")?.as_str()?;
                    let state_type = n.get("state").and_then(|s| s.get("type")).and_then(|t| t.as_str()).unwrap_or("");
                    // Solo advertir de issues activas
                    if state_type == "completed" || state_type == "canceled" {
                        return None;
                    }
                    Some(format!("  {id} \"{t}\""))
                })
                .collect();

            if !similar.is_empty() {
                eprintln!("⚠ Issues similares encontradas:");
                for s in &similar {
                    eprintln!("{s}");
                }
                eprintln!("Creando de todos modos. Usa --force para omitir esta comprobación.");
            }
        }
    }
    // Si la búsqueda falla, no bloquear la creación
    Ok(())
}

/// Wrapper público para que update.rs pueda usarlo
pub fn parse_due_date_pub(input: &str) -> Result<String, String> {
    parse_due_date(input)
}

/// Parsea una fecha de due date (ISO, relativa, +Nd)
fn parse_due_date(input: &str) -> Result<String, String> {
    use chrono::{Datelike, NaiveDate, Utc, Weekday};

    let today = Utc::now().date_naive();

    // ISO date
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(date.format("%Y-%m-%d").to_string());
    }

    // +Nd (relative days)
    if let Some(rest) = input.strip_prefix('+') {
        if let Some(days_str) = rest.strip_suffix('d') {
            if let Ok(days) = days_str.parse::<i64>() {
                let date = today + chrono::Duration::days(days);
                return Ok(date.format("%Y-%m-%d").to_string());
            }
        }
        // +Nw (weeks)
        if let Some(weeks_str) = rest.strip_suffix('w') {
            if let Ok(weeks) = weeks_str.parse::<i64>() {
                let date = today + chrono::Duration::weeks(weeks);
                return Ok(date.format("%Y-%m-%d").to_string());
            }
        }
    }

    // Day names (próximo día de la semana)
    let weekday = match input.to_lowercase().as_str() {
        "monday" | "mon" => Some(Weekday::Mon),
        "tuesday" | "tue" => Some(Weekday::Tue),
        "wednesday" | "wed" => Some(Weekday::Wed),
        "thursday" | "thu" => Some(Weekday::Thu),
        "friday" | "fri" => Some(Weekday::Fri),
        "saturday" | "sat" => Some(Weekday::Sat),
        "sunday" | "sun" => Some(Weekday::Sun),
        _ => None,
    };

    if let Some(target) = weekday {
        let current = today.weekday();
        let days_ahead = (target.num_days_from_monday() as i64
            - current.num_days_from_monday() as i64
            + 7)
            % 7;
        let days_ahead = if days_ahead == 0 { 7 } else { days_ahead };
        let date = today + chrono::Duration::days(days_ahead);
        return Ok(date.format("%Y-%m-%d").to_string());
    }

    // "tomorrow"
    if input.eq_ignore_ascii_case("tomorrow") {
        let date = today + chrono::Duration::days(1);
        return Ok(date.format("%Y-%m-%d").to_string());
    }

    Err(format!(
        "Could not parse due date \"{input}\". Use: YYYY-MM-DD, +7d, +2w, friday, tomorrow"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_due_date_iso() {
        assert_eq!(parse_due_date("2026-04-01").unwrap(), "2026-04-01");
    }

    #[test]
    fn test_parse_due_date_relative_days() {
        let result = parse_due_date("+7d").unwrap();
        // Debe ser una fecha válida 7 días desde hoy
        let date = chrono::NaiveDate::parse_from_str(&result, "%Y-%m-%d").unwrap();
        let today = chrono::Utc::now().date_naive();
        assert_eq!((date - today).num_days(), 7);
    }

    #[test]
    fn test_parse_due_date_weekday() {
        use chrono::Datelike;
        let result = parse_due_date("friday").unwrap();
        let date = chrono::NaiveDate::parse_from_str(&result, "%Y-%m-%d").unwrap();
        assert_eq!(date.weekday(), chrono::Weekday::Fri);
    }

    #[test]
    fn test_parse_due_date_invalid() {
        assert!(parse_due_date("not-a-date").is_err());
    }
}
