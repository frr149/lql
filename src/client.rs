use crate::auth;
use serde_json::Value;
use std::thread;
use std::time::Duration;

const GRAPHQL_URL: &str = "https://api.linear.app/graphql";
const MAX_RETRIES: u32 = 3;

pub struct Client {
    api_key: String,
    http: reqwest::blocking::Client,
}

impl Client {
    pub fn new(api_key_ref: &str) -> Result<Self, String> {
        let api_key = auth::get_api_key(api_key_ref)?;
        let http = reqwest::blocking::Client::new();
        Ok(Self { api_key, http })
    }

    /// Ejecuta una query GraphQL con variables
    pub fn query(&self, query: &str, variables: Value) -> Result<Value, String> {
        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });

        let mut last_err = String::new();
        for attempt in 0..=MAX_RETRIES {
            let response = self
                .http
                .post(GRAPHQL_URL)
                .header("Authorization", &self.api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send();

            match response {
                Ok(resp) => {
                    let status = resp.status();

                    // Rate limit: retry con backoff
                    if status.as_u16() == 429 {
                        if attempt < MAX_RETRIES {
                            let delay = Duration::from_secs(2u64.pow(attempt + 1));
                            eprintln!(
                                "ℹ Rate limited (429), retrying in {}s...",
                                delay.as_secs()
                            );
                            thread::sleep(delay);
                            continue;
                        }
                        return Err(
                            "Rate limited by Linear API after 3 retries. Try again later."
                                .to_string(),
                        );
                    }

                    // Auth error
                    if status.as_u16() == 401 {
                        return Err(
                            "Authentication failed. Check your API key: lql doctor".to_string()
                        );
                    }

                    // Server error: retry con backoff
                    if status.is_server_error() {
                        if attempt < MAX_RETRIES {
                            let delay = Duration::from_secs(2u64.pow(attempt + 1));
                            eprintln!(
                                "ℹ Server error ({status}), retrying in {}s...",
                                delay.as_secs()
                            );
                            thread::sleep(delay);
                            continue;
                        }
                        return Err(format!(
                            "Linear API server error ({status}). Try again later."
                        ));
                    }

                    // Parsear respuesta
                    let json: Value = resp
                        .json()
                        .map_err(|e| format!("Could not parse Linear API response: {e}"))?;

                    // Comprobar errores GraphQL
                    if let Some(errors) = json.get("errors") {
                        if let Some(first) = errors.as_array().and_then(|a| a.first()) {
                            let msg = first
                                .get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("Unknown error");
                            return Err(format!("Linear API error: {msg}"));
                        }
                    }

                    return json
                        .get("data")
                        .cloned()
                        .ok_or_else(|| "Linear API response missing 'data' field".to_string());
                }
                Err(e) => {
                    if attempt < MAX_RETRIES && e.is_connect() {
                        let delay = Duration::from_secs(2u64.pow(attempt + 1));
                        eprintln!("ℹ Connection error, retrying in {}s...", delay.as_secs());
                        thread::sleep(delay);
                        last_err = e.to_string();
                        continue;
                    }
                    return Err(format!(
                        "Could not connect to Linear API. Check your network.\n  Detail: {e}"
                    ));
                }
            }
        }
        Err(format!("Failed after {MAX_RETRIES} retries: {last_err}"))
    }

    /// Shortcut: query sin variables
    pub fn query_no_vars(&self, query: &str) -> Result<Value, String> {
        self.query(query, serde_json::json!({}))
    }
}

/// Metadata cacheada de Linear (teams, states, labels, projects)
/// Se fetchea una vez por ejecución y se reutiliza
#[derive(Debug, Clone)]
pub struct LinearMeta {
    pub teams: Vec<TeamInfo>,
    pub labels: Vec<LabelInfo>,
}

#[derive(Debug, Clone)]
pub struct TeamInfo {
    pub id: String,
    pub key: String,
    pub name: String,
    pub states: Vec<StateInfo>,
    pub projects: Vec<ProjectInfo>,
}

#[derive(Debug, Clone)]
pub struct StateInfo {
    pub id: String,
    pub name: String,
    pub state_type: String,
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct LabelInfo {
    pub id: String,
    pub name: String,
}

impl LinearMeta {
    pub fn fetch(client: &Client) -> Result<Self, String> {
        let data = client.query_no_vars(crate::queries::META_QUERY)?;

        let teams = parse_teams(&data)?;
        let labels = parse_labels(&data)?;

        Ok(Self { teams, labels })
    }

    pub fn find_team(&self, key: &str) -> Result<&TeamInfo, String> {
        self.teams
            .iter()
            .find(|t| t.key.eq_ignore_ascii_case(key))
            .ok_or_else(|| {
                let available: Vec<&str> = self.teams.iter().map(|t| t.key.as_str()).collect();
                format!(
                    "Team \"{key}\" not found. Available: {}",
                    available.join(", ")
                )
            })
    }

    pub fn find_state<'a>(&self, team: &'a TeamInfo, state_type: &str) -> Option<&'a StateInfo> {
        team.states.iter().find(move |s| s.state_type == state_type)
    }

    pub fn find_state_by_type_list<'a>(
        &'a self,
        team: &'a TeamInfo,
        state_types: &[String],
    ) -> Vec<&'a StateInfo> {
        team.states
            .iter()
            .filter(|s| state_types.iter().any(|t| t == &s.state_type))
            .collect()
    }

    pub fn find_label(&self, name: &str) -> Result<&LabelInfo, String> {
        self.labels
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                let available: Vec<&str> = self.labels.iter().map(|l| l.name.as_str()).collect();
                // Buscar similar (distancia de edición simple)
                let similar = find_similar(name, &available);
                let mut msg = format!("Label \"{name}\" not found.");
                if !similar.is_empty() {
                    msg.push_str(&format!(" Similar: {}", similar.join(", ")));
                }
                msg.push_str(&format!(
                    "\n  Available: {}",
                    available.join(", ")
                ));
                msg
            })
    }

    pub fn find_project<'a>(&self, team: &'a TeamInfo, name: &str) -> Result<&'a ProjectInfo, String> {
        // Rechazar IDs numéricos
        if name.chars().all(|c| c.is_ascii_digit()) {
            let available: Vec<&str> = team.projects.iter().map(|p| p.name.as_str()).collect();
            return Err(format!(
                "Use project name, not ID. Available: {}",
                available.join(", ")
            ));
        }

        team.projects
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                let available: Vec<&str> =
                    team.projects.iter().map(|p| p.name.as_str()).collect();
                format!(
                    "Project \"{name}\" not found. Available: {}",
                    available.join(", ")
                )
            })
    }
}

fn parse_teams(data: &Value) -> Result<Vec<TeamInfo>, String> {
    let teams_array = data
        .get("teams")
        .and_then(|t| t.get("nodes"))
        .and_then(|n| n.as_array())
        .ok_or("Could not parse teams from Linear API")?;

    let mut teams = Vec::new();
    for t in teams_array {
        let id = get_str(t, "id")?;
        let key = get_str(t, "key")?;
        let name = get_str(t, "name")?;

        let states = t
            .get("states")
            .and_then(|s| s.get("nodes"))
            .and_then(|n| n.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| {
                        Some(StateInfo {
                            id: s.get("id")?.as_str()?.to_string(),
                            name: s.get("name")?.as_str()?.to_string(),
                            state_type: s.get("type")?.as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let projects = t
            .get("projects")
            .and_then(|p| p.get("nodes"))
            .and_then(|n| n.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| {
                        Some(ProjectInfo {
                            id: p.get("id")?.as_str()?.to_string(),
                            name: p.get("name")?.as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        teams.push(TeamInfo {
            id,
            key,
            name,
            states,
            projects,
        });
    }
    Ok(teams)
}

fn parse_labels(data: &Value) -> Result<Vec<LabelInfo>, String> {
    let labels_array = data
        .get("issueLabels")
        .and_then(|l| l.get("nodes"))
        .and_then(|n| n.as_array())
        .ok_or("Could not parse labels from Linear API")?;

    Ok(labels_array
        .iter()
        .filter_map(|l| {
            Some(LabelInfo {
                id: l.get("id")?.as_str()?.to_string(),
                name: l.get("name")?.as_str()?.to_string(),
            })
        })
        .collect())
}

fn get_str(val: &Value, key: &str) -> Result<String, String> {
    val.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing field '{key}' in API response"))
}

/// Busca labels similares (substring match simple)
fn find_similar<'a>(needle: &str, haystack: &[&'a str]) -> Vec<&'a str> {
    let needle_lower = needle.to_lowercase();
    haystack
        .iter()
        .filter(|h| {
            let h_lower = h.to_lowercase();
            h_lower.contains(&needle_lower)
                || needle_lower.contains(&h_lower)
                || levenshtein(&needle_lower, &h_lower) <= 3
        })
        .copied()
        .take(3)
        .collect()
}

/// Distancia de Levenshtein simple
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut matrix = vec![vec![0usize; b.len() + 1]; a.len() + 1];

    for (i, row) in matrix.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        matrix[0][j] = j;
    }

    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construye LinearMeta desde el fixture real meta.json
    fn meta_from_fixture() -> LinearMeta {
        let path = format!("{}/tests/fixtures/meta.json", env!("CARGO_MANIFEST_DIR"));
        let content = std::fs::read_to_string(&path).unwrap();
        let fixture: Value = serde_json::from_str(&content).unwrap();
        let data = &fixture["data"];
        let teams = parse_teams(data).unwrap();
        let labels = parse_labels(data).unwrap();
        LinearMeta { teams, labels }
    }

    // --- ERR-23..27: Label validation ---

    // ERR-23: label inexistente rechazado con sugerencias
    #[test]
    fn test_label_not_found_with_similar() {
        let meta = meta_from_fixture();
        let err = meta.find_label("tokamax").unwrap_err(); // similar a "tokamak"
        assert!(err.contains("not found"), "Should say not found: {err}");
        assert!(err.contains("Available:"), "Should list available: {err}");
        assert!(err.contains("tokamak"), "Should suggest similar: {err}");
    }

    // ERR-24: label completamente inventado
    #[test]
    fn test_label_invented_not_found() {
        let meta = meta_from_fixture();
        let err = meta.find_label("kubernetes").unwrap_err();
        assert!(err.contains("not found"));
    }

    // ERR-25: label inventado "qa"
    #[test]
    fn test_label_qa_not_found() {
        let meta = meta_from_fixture();
        assert!(meta.find_label("datadog").is_err());
    }

    // ERR-26: label inventado
    #[test]
    fn test_label_nonexistent() {
        let meta = meta_from_fixture();
        assert!(meta.find_label("zzz-no-existe").is_err());
    }

    // ERR-27: label existente funciona
    #[test]
    fn test_label_existing_found() {
        let meta = meta_from_fixture();
        let label = meta.find_label("tokamak").unwrap();
        assert_eq!(label.name, "tokamak");
    }

    // Label case-insensitive
    #[test]
    fn test_label_case_insensitive() {
        let meta = meta_from_fixture();
        assert!(meta.find_label("Tokamak").is_ok());
        assert!(meta.find_label("TOKAMAK").is_ok());
        assert!(meta.find_label("LQL").is_ok());
    }

    // --- ERR-29..33: Project resolution ---

    // ERR-29: project por nombre exacto
    #[test]
    fn test_project_exact_name() {
        let meta = meta_from_fixture();
        let prod = meta.find_team("PROD").unwrap();
        let project = meta.find_project(prod, "Tokamak").unwrap();
        assert_eq!(project.name, "Tokamak");
    }

    // ERR-30: project por nombre case-insensitive
    #[test]
    fn test_project_case_insensitive() {
        let meta = meta_from_fixture();
        let prod = meta.find_team("PROD").unwrap();
        assert!(meta.find_project(prod, "tokamak").is_ok());
        assert!(meta.find_project(prod, "TOKAMAK").is_ok());
    }

    // ERR-32: project inexistente
    #[test]
    fn test_project_not_found() {
        let meta = meta_from_fixture();
        let prod = meta.find_team("PROD").unwrap();
        let err = meta.find_project(prod, "Dashboard").unwrap_err();
        assert!(err.contains("not found"), "Should say not found: {err}");
        assert!(err.contains("Available:"), "Should list available: {err}");
    }

    // ERR-33: project ID numérico rechazado
    #[test]
    fn test_project_numeric_id_rejected() {
        let meta = meta_from_fixture();
        let prod = meta.find_team("PROD").unwrap();
        let err = meta.find_project(prod, "686615456359").unwrap_err();
        assert!(err.contains("Use project name, not ID"), "{err}");
    }

    // --- Teams ---

    // Team by key
    #[test]
    fn test_find_team_by_key() {
        let meta = meta_from_fixture();
        assert!(meta.find_team("PROD").is_ok());
        assert!(meta.find_team("TOOL").is_ok());
        assert!(meta.find_team("CONT").is_ok());
    }

    // Team case-insensitive
    #[test]
    fn test_find_team_case_insensitive() {
        let meta = meta_from_fixture();
        assert!(meta.find_team("prod").is_ok());
        assert!(meta.find_team("Prod").is_ok());
    }

    // Team not found
    #[test]
    fn test_find_team_not_found() {
        let meta = meta_from_fixture();
        let err = meta.find_team("NONEXISTENT").unwrap_err();
        assert!(err.contains("not found"));
        assert!(err.contains("Available:"));
    }

    // --- States ---

    // find_state by type
    #[test]
    fn test_find_state_backlog() {
        let meta = meta_from_fixture();
        let prod = meta.find_team("PROD").unwrap();
        let state = meta.find_state(prod, "backlog");
        assert!(state.is_some());
        assert_eq!(state.unwrap().state_type, "backlog");
    }

    #[test]
    fn test_find_state_completed() {
        let meta = meta_from_fixture();
        let prod = meta.find_team("PROD").unwrap();
        let state = meta.find_state(prod, "completed");
        assert!(state.is_some());
    }

    #[test]
    fn test_find_state_nonexistent() {
        let meta = meta_from_fixture();
        let prod = meta.find_team("PROD").unwrap();
        assert!(meta.find_state(prod, "nonexistent").is_none());
    }

    // --- Levenshtein ---

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("tokamak", "tokamak"), 0);
    }

    #[test]
    fn test_levenshtein_one_char() {
        assert_eq!(levenshtein("tokamak", "tokamac"), 1);
    }

    #[test]
    fn test_levenshtein_similar() {
        // "appstore" vs "autocorrect" = distance > 3
        assert!(levenshtein("appstore", "autocorrect") > 3);
    }

    // --- find_similar ---

    #[test]
    fn test_find_similar_substring() {
        let haystack = vec!["tokamak", "acme", "blog", "autocorrect"];
        let similar = find_similar("toka", &haystack);
        assert!(similar.contains(&"tokamak"));
    }

    #[test]
    fn test_find_similar_levenshtein() {
        let haystack = vec!["tokamak", "acme", "blog"];
        let similar = find_similar("tokamac", &haystack); // 1 edit distance
        assert!(similar.contains(&"tokamak"));
    }

    #[test]
    fn test_find_similar_no_match() {
        let haystack = vec!["tokamak", "acme", "blog"];
        let similar = find_similar("zzzzzzzzzzz", &haystack);
        assert!(similar.is_empty());
    }

    // --- Meta parsing from real fixture ---

    #[test]
    fn test_parse_meta_fixture_completeness() {
        let meta = meta_from_fixture();

        // Todos los teams del context-map deben existir
        for key in &["PROD", "CONT", "PRIV", "TOOL", "KC"] {
            assert!(
                meta.find_team(key).is_ok(),
                "Team {key} should exist in fixture"
            );
        }

        // Labels del context-map deben existir
        for label in &["tokamak", "acme", "blog", "lql"] {
            assert!(
                meta.find_label(label).is_ok(),
                "Label {label} should exist in fixture"
            );
        }
    }
}
