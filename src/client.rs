use crate::auth;
use serde_json::Value;
use std::thread;
use std::time::Duration;

const GRAPHQL_URL: &str = "https://api.linear.app/graphql";
const MAX_RETRIES: u32 = 3;

/// Acción a tomar tras evaluar un código HTTP
#[derive(Debug, PartialEq)]
enum HttpAction {
    /// Reintentar tras delay
    Retry(String),
    /// Error fatal, devolver al caller
    Error(String),
    /// Continuar procesando el body
    Continue,
}

/// Clasifica un status HTTP y decide si reintentar, abortar o continuar
fn classify_http_status(status: u16, attempt: u32, max_retries: u32) -> HttpAction {
    match status {
        429 => {
            if attempt < max_retries {
                HttpAction::Retry(format!(
                    "ℹ Rate limited (429), retrying in {}s...",
                    2u64.pow(attempt + 1)
                ))
            } else {
                HttpAction::Error(
                    "Rate limited by Linear API after 3 retries. Try again later.".to_string(),
                )
            }
        }
        401 => HttpAction::Error(
            "Authentication failed. Check your API key: lql doctor".to_string(),
        ),
        500..=599 => {
            if attempt < max_retries {
                HttpAction::Retry(format!(
                    "ℹ Server error ({status}), retrying in {}s...",
                    2u64.pow(attempt + 1)
                ))
            } else {
                HttpAction::Error(format!(
                    "Linear API server error ({status}). Try again later."
                ))
            }
        }
        _ => HttpAction::Continue,
    }
}

/// Parsea el body JSON de una respuesta Linear y extrae el campo "data"
fn handle_response(body: &str) -> Result<Value, String> {
    let json: Value =
        serde_json::from_str(body).map_err(|e| format!("Could not parse Linear API response: {e}"))?;

    if let Some(errors) = json.get("errors") {
        if let Some(first) = errors.as_array().and_then(|a| a.first()) {
            let msg = first
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            return Err(format!("Linear API error: {msg}"));
        }
    }

    json.get("data")
        .cloned()
        .ok_or_else(|| "Linear API response missing 'data' field".to_string())
}

/// Trait para ejecutar queries GraphQL — permite mocking en tests
pub trait GraphQLClient {
    fn query(&self, query: &str, variables: Value) -> Result<Value, String>;

    fn query_no_vars(&self, query: &str) -> Result<Value, String> {
        self.query(query, serde_json::json!({}))
    }
}

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
}

impl GraphQLClient for Client {
    fn query(&self, query: &str, variables: Value) -> Result<Value, String> {
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
                    let status = resp.status().as_u16();
                    match classify_http_status(status, attempt, MAX_RETRIES) {
                        HttpAction::Retry(msg) => {
                            let delay = Duration::from_secs(2u64.pow(attempt + 1));
                            eprintln!("{msg}");
                            thread::sleep(delay);
                            continue;
                        }
                        HttpAction::Error(msg) => return Err(msg),
                        HttpAction::Continue => {}
                    }

                    let text = resp
                        .text()
                        .map_err(|e| format!("Could not read Linear API response: {e}"))?;
                    return handle_response(&text);
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
}

/// Metadata cacheada de Linear (teams, states, labels, projects)
/// Se fetchea una vez por ejecución y se reutiliza
#[derive(Debug, Clone)]
pub struct LinearMeta {
    pub teams: Vec<TeamInfo>,
    pub labels: Vec<LabelInfo>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
    pub fn fetch(client: &dyn GraphQLClient) -> Result<Self, String> {
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
                // Buscar sugerencia por Levenshtein
                let needle = key.to_lowercase();
                let mut scored: Vec<(&str, usize)> = available
                    .iter()
                    .map(|&h| (h, levenshtein(&needle, &h.to_lowercase())))
                    .collect();
                scored.sort_by_key(|&(_, d)| d);
                if let Some(&(best, dist)) = scored.first() {
                    if dist <= 3 {
                        return format!(
                            "Team \"{key}\" does not exist. Did you mean: {best}?"
                        );
                    }
                }
                format!(
                    "Team \"{key}\" not found. Available: {}",
                    available.join(", ")
                )
            })
    }

    pub fn find_state<'a>(&self, team: &'a TeamInfo, state_type: &str) -> Option<&'a StateInfo> {
        team.states.iter().find(move |s| s.state_type == state_type)
    }



    pub fn find_label(&self, name: &str) -> Result<&LabelInfo, String> {
        self.labels
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                let needle = name.to_lowercase();
                let mut scored: Vec<(&str, usize)> = self
                    .labels
                    .iter()
                    .map(|l| {
                        (
                            l.name.as_str(),
                            levenshtein(&needle, &l.name.to_lowercase()),
                        )
                    })
                    .collect();
                scored.sort_by_key(|&(_, d)| d);
                let top: Vec<&str> = scored.iter().take(10).map(|&(s, _)| s).collect();
                format!(
                    "Label \"{name}\" not found. Closest (of {}): {}",
                    self.labels.len(),
                    top.join(", ")
                )
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
        assert!(err.contains("Closest (of"), "Should list closest: {err}");
        assert!(err.contains("tokamak"), "Should suggest similar: {err}");
    }

    // ERR-23b: el error trunca a 10 labels máximo
    #[test]
    fn test_label_error_truncated() {
        let meta = meta_from_fixture();
        let err = meta.find_label("kubernetes").unwrap_err();
        // No debe volcar todos los labels — solo hasta 10
        let comma_count = err.matches(',').count();
        assert!(
            comma_count <= 9,
            "Should show at most 10 labels (9 commas), got {comma_count} commas: {err}"
        );
        assert!(err.contains("Closest (of"), "Should show total count: {err}");
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

    // ERR-31: project con espacios case-insensitive
    #[test]
    fn test_project_with_spaces_case_insensitive() {
        let meta = meta_from_fixture();
        let tool = meta.find_team("TOOL").unwrap();
        // "Social Publisher" existe en TOOL fixture
        assert!(meta.find_project(tool, "social publisher").is_ok());
        assert!(meta.find_project(tool, "SOCIAL PUBLISHER").is_ok());
        assert!(meta.find_project(tool, "Social Publisher").is_ok());
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

    // Team not found (lejano, sin sugerencia)
    #[test]
    fn test_find_team_not_found() {
        let meta = meta_from_fixture();
        let err = meta.find_team("NONEXISTENT").unwrap_err();
        assert!(err.contains("not found"));
        assert!(err.contains("Available:"));
    }

    // ERR-36: --team BLO → "Did you mean: BLO?" (existe como team en fixture)
    // Nota: BLO existe en Linear pero no en los 5 teams del context-map.
    // Sin embargo sí está en el fixture de meta. Se sugiere el match más cercano.
    #[test]
    fn test_find_team_blo_suggestion() {
        let meta = meta_from_fixture();
        // BLO existe en el fixture, así que find_team lo encuentra
        // Este test verifica que el fixture incluye BLO
        assert!(meta.find_team("BLO").is_ok());
    }

    // ERR-37: team cercano con sugerencia
    #[test]
    fn test_find_team_suggestion_levenshtein() {
        let meta = meta_from_fixture();
        let err = meta.find_team("PORD").unwrap_err(); // typo de PROD
        assert!(err.contains("Did you mean"), "{err}");
        assert!(err.contains("PROD"), "{err}");
    }

    // ERR-38: team lejano sin sugerencia
    #[test]
    fn test_find_team_no_suggestion_far() {
        let meta = meta_from_fixture();
        let err = meta.find_team("ZZZZZZZ").unwrap_err();
        assert!(err.contains("Available:"), "{err}");
        assert!(!err.contains("Did you mean"), "{err}");
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

    // --- ERR-48..52: API error handling ---

    // ERR-48: GraphQL error parseado correctamente
    #[test]
    fn test_handle_response_graphql_error() {
        let body = r#"{"errors":[{"message":"Entity not found"}]}"#;
        let err = handle_response(body).unwrap_err();
        assert!(err.contains("Linear API error: Entity not found"), "{err}");
    }

    // ERR-48b: GraphQL error sin message
    #[test]
    fn test_handle_response_graphql_error_no_message() {
        let body = r#"{"errors":[{}]}"#;
        let err = handle_response(body).unwrap_err();
        assert!(err.contains("Unknown error"), "{err}");
    }

    // ERR-48c: respuesta válida devuelve data
    #[test]
    fn test_handle_response_valid() {
        let body = r#"{"data":{"issues":{"nodes":[]}}}"#;
        let data = handle_response(body).unwrap();
        assert!(data.get("issues").is_some());
    }

    // ERR-48d: respuesta sin data
    #[test]
    fn test_handle_response_missing_data() {
        let body = r#"{"something":"else"}"#;
        let err = handle_response(body).unwrap_err();
        assert!(err.contains("missing 'data' field"), "{err}");
    }

    // ERR-48e: body no es JSON válido
    #[test]
    fn test_handle_response_invalid_json() {
        let err = handle_response("not json").unwrap_err();
        assert!(err.contains("Could not parse"), "{err}");
    }

    // ERR-49: 429 rate limit — primeros intentos reintentables, último es error
    #[test]
    fn test_classify_429_retry() {
        assert!(matches!(
            classify_http_status(429, 0, 3),
            HttpAction::Retry(_)
        ));
        assert!(matches!(
            classify_http_status(429, 2, 3),
            HttpAction::Retry(_)
        ));
    }

    #[test]
    fn test_classify_429_exhausted() {
        let result = classify_http_status(429, 3, 3);
        assert!(matches!(result, HttpAction::Error(ref msg) if msg.contains("Rate limited")));
    }

    // ERR-50: 401 auth failed — siempre error inmediato
    #[test]
    fn test_classify_401() {
        let result = classify_http_status(401, 0, 3);
        assert!(matches!(result, HttpAction::Error(ref msg) if msg.contains("Authentication failed")));
    }

    // ERR-51: 500 server error — reintenta, luego error
    #[test]
    fn test_classify_500_retry() {
        assert!(matches!(
            classify_http_status(500, 0, 3),
            HttpAction::Retry(_)
        ));
    }

    #[test]
    fn test_classify_500_exhausted() {
        let result = classify_http_status(500, 3, 3);
        assert!(matches!(result, HttpAction::Error(ref msg) if msg.contains("server error")));
    }

    // ERR-51b: otros 5xx también reintentan
    #[test]
    fn test_classify_502_503() {
        assert!(matches!(
            classify_http_status(502, 0, 3),
            HttpAction::Retry(_)
        ));
        assert!(matches!(
            classify_http_status(503, 0, 3),
            HttpAction::Retry(_)
        ));
    }

    // ERR-52: el mensaje de network error se verifica en formato
    // (no podemos simular reqwest::Error sin mock, pero testeamos classify para 200 = Continue)
    #[test]
    fn test_classify_200_continues() {
        assert_eq!(classify_http_status(200, 0, 3), HttpAction::Continue);
    }

    #[test]
    fn test_classify_other_status_continues() {
        assert_eq!(classify_http_status(204, 0, 3), HttpAction::Continue);
        assert_eq!(classify_http_status(301, 0, 3), HttpAction::Continue);
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
        for label in &["tokamak", "qinqin", "blog", "lql"] {
            assert!(
                meta.find_label(label).is_ok(),
                "Label {label} should exist in fixture"
            );
        }
    }
}
