use crate::cli::{self, CreateOpts};
use crate::client::{Client, GraphQLClient, LinearMeta};
use crate::config::Config;
use crate::format;
use std::io::{IsTerminal, Read};

pub fn run(config: &Config, opts: &CreateOpts) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Could not get cwd: {e}"))?;
    let client = Client::new(&config.auth.api_key_ref)?;
    let meta = LinearMeta::fetch(&client)?;

    // Resolver team/project/label
    let title = opts.resolved_title()?;
    let (team_key, ctx_project, ctx_label) = config.resolve_team(opts.team.as_deref(), &cwd)?;
    let team = meta.find_team(&team_key)?;

    // Construir input de la mutación
    let mut input = serde_json::json!({
        "title": title,
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
            let label = meta.find_label_for_team(team, name)?;
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
        check_duplicates(&client, title)?;
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
    get_description_from_args(opts.description.as_ref(), opts.description_file.as_ref())
}

pub fn get_description_from_args(
    description: Option<&String>,
    description_file: Option<&String>,
) -> Result<Option<String>, String> {
    // Inline
    if let Some(desc) = description {
        return Ok(Some(desc.clone()));
    }

    // Fichero
    if let Some(path) = description_file {
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

fn check_duplicates(client: &dyn GraphQLClient, title: &str) -> Result<(), String> {
    let variables = serde_json::json!({
        "term": title,
        "first": 5,
    });

    if let Ok(data) = client.query(crate::queries::SEARCH_QUERY, variables)
        && let Some(nodes) = data
            .get("searchIssues")
            .and_then(|s| s.get("nodes"))
            .and_then(|n| n.as_array())
    {
        let similar: Vec<String> = nodes
            .iter()
            .filter_map(|n| {
                let id = n.get("identifier")?.as_str()?;
                let t = n.get("title")?.as_str()?;
                let state_type = n
                    .get("state")
                    .and_then(|s| s.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                // Solo advertir de issues activas
                if state_type == "completed" || state_type == "canceled" {
                    return None;
                }
                Some(format!("  {id} \"{t}\""))
            })
            .collect();

        if !similar.is_empty() {
            eprintln!("⚠ Similar issues found:");
            for s in &similar {
                eprintln!("{s}");
            }
            eprintln!("Creating anyway. Use --force to skip this check.");
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
        if let Some(days_str) = rest.strip_suffix('d')
            && let Ok(days) = days_str.parse::<i64>()
        {
            let date = today + chrono::Duration::days(days);
            return Ok(date.format("%Y-%m-%d").to_string());
        }
        // +Nw (weeks)
        if let Some(weeks_str) = rest.strip_suffix('w')
            && let Ok(weeks) = weeks_str.parse::<i64>()
        {
            let date = today + chrono::Duration::weeks(weeks);
            return Ok(date.format("%Y-%m-%d").to_string());
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
        let days_ahead =
            (target.num_days_from_monday() as i64 - current.num_days_from_monday() as i64 + 7) % 7;
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
    use crate::client::GraphQLClient;

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

    #[test]
    fn test_parse_due_date_tomorrow() {
        let result = parse_due_date("tomorrow").unwrap();
        let date = chrono::NaiveDate::parse_from_str(&result, "%Y-%m-%d").unwrap();
        let tomorrow = chrono::Utc::now().date_naive() + chrono::Duration::days(1);
        assert_eq!(date, tomorrow);
    }

    #[test]
    fn test_parse_due_date_weeks() {
        let result = parse_due_date("+2w").unwrap();
        let date = chrono::NaiveDate::parse_from_str(&result, "%Y-%m-%d").unwrap();
        let expected = chrono::Utc::now().date_naive() + chrono::Duration::weeks(2);
        assert_eq!(date, expected);
    }

    // --- ERR-39..45: Escapado seguro con serde ---

    // ERR-39: descripción con comillas dobles
    #[test]
    fn test_escape_double_quotes() {
        let desc = r#"El campo "title" no se escapa"#;
        let json = serde_json::json!({"description": desc});
        let serialized = serde_json::to_string(&json).unwrap();
        // serde escapa las comillas correctamente
        assert!(serialized.contains(r#"\"title\""#));
        // Y se deserializa al valor original
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["description"].as_str().unwrap(), desc);
    }

    // ERR-40: descripción con backticks
    #[test]
    fn test_escape_backticks() {
        let desc = "Usar `json.dumps()` para escapar";
        let json = serde_json::json!({"description": desc});
        let serialized = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["description"].as_str().unwrap(), desc);
    }

    // ERR-41: descripción con newlines
    #[test]
    fn test_escape_newlines() {
        let desc = "## Problema\n\nEl token expira.\n\n## Fix\n\nDetectar expiración.";
        let json = serde_json::json!({"description": desc});
        let serialized = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["description"].as_str().unwrap(), desc);
    }

    // ERR-42: descripción con $variables (no expandidas)
    #[test]
    fn test_escape_dollar_variables() {
        let desc = "Set $PATH to include ~/.local/bin";
        let json = serde_json::json!({"description": desc});
        let serialized = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["description"].as_str().unwrap(), desc);
    }

    // ERR-43: descripción con backslashes
    #[test]
    fn test_escape_backslashes() {
        let desc = r"Regex: \d+\.\d+";
        let json = serde_json::json!({"description": desc});
        let serialized = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["description"].as_str().unwrap(), desc);
    }

    // ERR-44: descripción con emojis y unicode
    #[test]
    fn test_escape_unicode_emoji() {
        let desc = "⚠️ Error en producción — 日本語テスト";
        let json = serde_json::json!({"description": desc});
        let serialized = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["description"].as_str().unwrap(), desc);
    }

    // ERR-45: heredoc con comillas, backticks y $variables mezclados
    #[test]
    fn test_escape_mixed_special_chars() {
        let desc =
            "## Problema\nEl campo \"title\" tiene `backticks` y $variables.\nPath: C:\\Users\\foo";
        let json = serde_json::json!({"description": desc});
        let serialized = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["description"].as_str().unwrap(), desc);
    }

    // Verificar que la query GraphQL completa con variables es JSON válido
    #[test]
    fn test_full_graphql_body_valid_json() {
        let title = r#"Fix "auth" token's $refresh — ¡urgente!"#;
        let desc = "## Steps\n1. Check `expiresAt`\n2. Set $PATH\n3. Regex: \\d+\n\n> Quote with \"double quotes\"";

        let variables = serde_json::json!({
            "input": {
                "title": title,
                "description": desc,
                "teamId": "some-uuid",
                "priority": 1,
            }
        });

        let body = serde_json::json!({
            "query": "mutation($input: IssueCreateInput!) { issueCreate(input: $input) { success } }",
            "variables": variables,
        });

        // El body completo debe ser JSON válido
        let serialized = serde_json::to_string(&body).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        // Los valores deben sobrevivir el round-trip
        assert_eq!(
            parsed["variables"]["input"]["title"].as_str().unwrap(),
            title
        );
        assert_eq!(
            parsed["variables"]["input"]["description"]
                .as_str()
                .unwrap(),
            desc
        );
    }

    // --- ERR-72/73: Duplicate detection con mock ---

    struct MockClient {
        response: Result<serde_json::Value, String>,
    }

    impl GraphQLClient for MockClient {
        fn query(
            &self,
            _query: &str,
            _variables: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            self.response.clone()
        }
    }

    // ERR-72: duplicados encontrados genera warning (no error)
    #[test]
    fn test_check_duplicates_found() {
        let client = MockClient {
            response: Ok(serde_json::json!({
                "searchIssues": {"nodes": [{
                    "identifier": "PROD-100",
                    "title": "OAuth token refresh",
                    "state": {"type": "unstarted"}
                }]}
            })),
        };
        // check_duplicates siempre devuelve Ok — solo emite warning a stderr
        let result = check_duplicates(&client, "OAuth token refresh");
        assert!(result.is_ok());
    }

    // ERR-72b: duplicados completados no generan warning
    #[test]
    fn test_check_duplicates_completed_ignored() {
        let client = MockClient {
            response: Ok(serde_json::json!({
                "searchIssues": {"nodes": [{
                    "identifier": "PROD-100",
                    "title": "OAuth token refresh",
                    "state": {"type": "completed"}
                }]}
            })),
        };
        assert!(check_duplicates(&client, "OAuth token refresh").is_ok());
    }

    // ERR-73: si la búsqueda falla, no bloquea la creación
    #[test]
    fn test_check_duplicates_search_error_does_not_block() {
        let client = MockClient {
            response: Err("Connection error".to_string()),
        };
        assert!(check_duplicates(&client, "Some title").is_ok());
    }

    // Sin duplicados
    #[test]
    fn test_check_duplicates_none_found() {
        let client = MockClient {
            response: Ok(serde_json::json!({
                "searchIssues": {"nodes": []}
            })),
        };
        assert!(check_duplicates(&client, "Unique title").is_ok());
    }
}
