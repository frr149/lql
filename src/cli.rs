use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "lql",
    version,
    about = "Linear Query Language — because everything must be rewritten in Rust 🦀"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Listar issues
    List(ListOpts),
    /// Crear issue
    Create(CreateOpts),
    /// Actualizar issue
    Update(UpdateOpts),
    /// Ver detalle de issue
    View(ViewOpts),
    /// Buscar issues por texto
    Search(SearchOpts),
    /// Añadir comentario a issue
    Comment(CommentOpts),
    /// Crear relación entre issues
    Relate(RelateOpts),
    /// Listar labels disponibles
    Labels(LabelsOpts),
    /// Validar config, auth, teams y labels
    Doctor,
    /// Mostrar resolución del cwd (team/project/label)
    Context,
    /// Ejecutar query GraphQL directa
    Raw(RawOpts),
}

#[derive(Parser)]
pub struct ListOpts {
    /// Filtrar por estado (acepta nombres UI: Todo, Done, "In Progress")
    #[arg(long, alias = "status", value_delimiter = ',')]
    pub state: Option<Vec<String>>,

    /// Filtrar por label
    #[arg(long)]
    pub label: Option<Vec<String>>,

    /// Filtrar por proyecto (case-insensitive)
    #[arg(long)]
    pub project: Option<String>,

    /// Filtrar por team (override auto-detect)
    #[arg(long)]
    pub team: Option<String>,

    /// Solo issues vencidas
    #[arg(long)]
    pub overdue: bool,

    /// Todos los teams (ignorar context-map)
    #[arg(long)]
    pub all_teams: bool,

    /// Ordenar por (default: priority)
    #[arg(long, default_value = "priority")]
    pub sort: String,

    /// Límite de resultados
    #[arg(long)]
    pub limit: Option<u32>,

    /// Sin límite (todos los resultados)
    #[arg(long)]
    pub all: bool,

    /// Output en JSONL
    #[arg(long)]
    pub json: bool,

    /// Filtrar por prioridad (acepta nombres: urgent, high, medium, low)
    #[arg(long)]
    pub priority: Option<String>,

    // Flags ignorados silenciosamente (compatibilidad CLI oficial)
    #[arg(long, hide = true)]
    pub no_pager: bool,
    #[arg(long, hide = true)]
    pub no_interactive: bool,
}

#[derive(Parser)]
pub struct CreateOpts {
    /// Título de la issue
    pub title: String,

    /// Descripción inline
    #[arg(short, long)]
    pub description: Option<String>,

    /// Descripción desde fichero
    #[arg(long)]
    pub description_file: Option<String>,

    /// Team (override auto-detect)
    #[arg(long)]
    pub team: Option<String>,

    /// Proyecto (case-insensitive)
    #[arg(long)]
    pub project: Option<String>,

    /// Label
    #[arg(long)]
    pub label: Option<Vec<String>>,

    /// Prioridad (nombre o número)
    #[arg(long)]
    pub priority: Option<String>,

    /// Fecha de vencimiento (ISO, relativa: friday, +7d)
    #[arg(long)]
    pub due: Option<String>,

    /// Estado inicial
    #[arg(long, alias = "status")]
    pub state: Option<String>,

    /// Omitir detección de duplicados
    #[arg(long)]
    pub force: bool,

    /// Output en JSON
    #[arg(long)]
    pub json: bool,

    // Flags ignorados silenciosamente
    #[arg(long, hide = true)]
    pub no_pager: bool,
    #[arg(long, hide = true)]
    pub no_interactive: bool,
}

#[derive(Parser)]
pub struct UpdateOpts {
    /// Issue ID (ej: PROD-587)
    pub issue_id: String,

    /// Cambiar estado
    #[arg(long, alias = "status")]
    pub state: Option<String>,

    /// Cambiar prioridad
    #[arg(long)]
    pub priority: Option<String>,

    /// Cambiar proyecto
    #[arg(long)]
    pub project: Option<String>,

    /// Añadir label
    #[arg(long)]
    pub label: Option<Vec<String>>,

    /// Cambiar título
    #[arg(long)]
    pub title: Option<String>,

    /// Descripción inline
    #[arg(short, long)]
    pub description: Option<String>,

    /// Descripción desde fichero
    #[arg(long)]
    pub description_file: Option<String>,

    /// Cambiar due date
    #[arg(long)]
    pub due: Option<String>,

    /// Output en JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser)]
pub struct ViewOpts {
    /// Issue ID (ej: PROD-587)
    pub issue_id: String,

    /// Output en JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser)]
pub struct SearchOpts {
    /// Término de búsqueda
    pub query: String,

    /// Filtrar por team
    #[arg(long)]
    pub team: Option<String>,

    /// Filtrar por estado
    #[arg(long, alias = "status", value_delimiter = ',')]
    pub state: Option<Vec<String>>,

    /// Límite de resultados
    #[arg(long)]
    pub limit: Option<u32>,

    /// Output en JSONL
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser)]
pub struct CommentOpts {
    /// Issue ID (ej: PROD-587)
    pub issue_id: String,

    /// Texto del comentario
    pub body: Option<String>,

    /// Comentario desde fichero
    #[arg(long)]
    pub file: Option<String>,
}

#[derive(Parser)]
pub struct RelateOpts {
    /// Issue origen (ej: PROD-587)
    pub from: String,

    /// Tipo de relación: blocks, blocked-by, related
    pub relation_type: String,

    /// Issue destino (ej: PROD-588)
    pub to: String,
}

#[derive(Parser)]
pub struct LabelsOpts {
    /// Filtrar por team
    #[arg(long)]
    pub team: Option<String>,

    /// Output en JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser)]
pub struct RawOpts {
    /// Query GraphQL inline
    pub query: Option<String>,

    /// Query desde fichero
    #[arg(long)]
    pub file: Option<String>,

    /// Variable key=value (repetible)
    #[arg(long = "var")]
    pub vars: Option<Vec<String>>,

    /// Variables desde fichero JSON
    #[arg(long)]
    pub vars_file: Option<String>,
}

pub fn parse() -> Cli {
    Cli::parse()
}

/// Normaliza un valor de estado (UI names → API values)
pub fn normalize_state(state: &str, aliases: &std::collections::HashMap<String, String>) -> String {
    // Intentar lookup directo
    if let Some(normalized) = aliases.get(state) {
        eprintln!("ℹ State \"{state}\" → normalized to \"{normalized}\"");
        return normalized.clone();
    }
    // Intentar case-insensitive
    for (key, val) in aliases {
        if key.eq_ignore_ascii_case(state) {
            eprintln!("ℹ State \"{state}\" → normalized to \"{val}\"");
            return val.clone();
        }
    }
    // Si ya es un valor API válido, devolver tal cual
    let api_values = ["backlog", "unstarted", "started", "completed", "canceled"];
    let lower = state.to_lowercase();
    if api_values.contains(&lower.as_str()) {
        return lower;
    }
    // Devolver tal cual y dejar que la API lo rechace
    state.to_lowercase()
}

/// Normaliza un valor de prioridad (nombre → número)
pub fn normalize_priority(
    priority: &str,
    aliases: &std::collections::HashMap<String, u8>,
) -> Result<u8, String> {
    // Intentar parsear como número directamente
    if let Ok(n) = priority.parse::<u8>() {
        if n <= 4 {
            return Ok(n);
        }
        return Err(format!("Priority must be 0-4, got {n}"));
    }
    // Intentar lookup
    let lower = priority.to_lowercase();
    if let Some(&n) = aliases.get(&lower) {
        eprintln!("ℹ Priority \"{priority}\" → normalized to {n}");
        return Ok(n);
    }
    Err(format!(
        "Unknown priority \"{priority}\". Available: urgent (1), high (2), medium (3), low (4), none (0)"
    ))
}

/// Normaliza el valor de sort
pub fn normalize_sort(sort: &str) -> String {
    match sort.to_lowercase().as_str() {
        "updated" | "updatedat" => {
            if sort.to_lowercase() == "updated" {
                eprintln!("ℹ Sort \"updated\" → normalized to \"updatedAt\"");
            }
            "updatedAt".to_string()
        }
        "created" | "createdat" => "createdAt".to_string(),
        "priority" => "priority".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn state_aliases() -> HashMap<String, String> {
        HashMap::from([
            ("Todo".to_string(), "unstarted".to_string()),
            ("In Progress".to_string(), "started".to_string()),
            ("Done".to_string(), "completed".to_string()),
            ("Canceled".to_string(), "canceled".to_string()),
            ("Cancelled".to_string(), "canceled".to_string()),
        ])
    }

    fn priority_aliases() -> HashMap<String, u8> {
        HashMap::from([
            ("urgent".to_string(), 1),
            ("high".to_string(), 2),
            ("medium".to_string(), 3),
            ("low".to_string(), 4),
            ("none".to_string(), 0),
        ])
    }

    // ERR-03: --state Todo → unstarted
    #[test]
    fn test_normalize_state_todo() {
        assert_eq!(normalize_state("Todo", &state_aliases()), "unstarted");
    }

    // ERR-04: --state "In Progress" → started
    #[test]
    fn test_normalize_state_in_progress() {
        assert_eq!(
            normalize_state("In Progress", &state_aliases()),
            "started"
        );
    }

    // ERR-05: --state Done → completed
    #[test]
    fn test_normalize_state_done() {
        assert_eq!(normalize_state("Done", &state_aliases()), "completed");
    }

    // ERR-06: --state cancelled (doble L) → canceled
    #[test]
    fn test_normalize_state_cancelled_double_l() {
        assert_eq!(normalize_state("Cancelled", &state_aliases()), "canceled");
    }

    // API values pasan sin cambio
    #[test]
    fn test_normalize_state_api_value_passthrough() {
        assert_eq!(normalize_state("backlog", &state_aliases()), "backlog");
        assert_eq!(normalize_state("started", &state_aliases()), "started");
    }

    // Case insensitive
    #[test]
    fn test_normalize_state_case_insensitive() {
        assert_eq!(normalize_state("todo", &state_aliases()), "unstarted");
        assert_eq!(normalize_state("TODO", &state_aliases()), "unstarted");
        assert_eq!(normalize_state("done", &state_aliases()), "completed");
    }

    // ERR-07: --priority urgent → 1
    #[test]
    fn test_normalize_priority_urgent() {
        assert_eq!(normalize_priority("urgent", &priority_aliases()), Ok(1));
    }

    // ERR-08: --priority high → 2
    #[test]
    fn test_normalize_priority_high() {
        assert_eq!(normalize_priority("high", &priority_aliases()), Ok(2));
    }

    // ERR-09: --priority medium → 3
    #[test]
    fn test_normalize_priority_medium() {
        assert_eq!(normalize_priority("medium", &priority_aliases()), Ok(3));
    }

    // ERR-10: --priority low → 4
    #[test]
    fn test_normalize_priority_low() {
        assert_eq!(normalize_priority("low", &priority_aliases()), Ok(4));
    }

    // Número directo
    #[test]
    fn test_normalize_priority_number() {
        assert_eq!(normalize_priority("1", &priority_aliases()), Ok(1));
        assert_eq!(normalize_priority("0", &priority_aliases()), Ok(0));
    }

    // Prioridad inválida
    #[test]
    fn test_normalize_priority_invalid() {
        assert!(normalize_priority("critical", &priority_aliases()).is_err());
        assert!(normalize_priority("5", &priority_aliases()).is_err());
    }

    // ERR-13: --sort updated → updatedAt
    #[test]
    fn test_normalize_sort_updated() {
        assert_eq!(normalize_sort("updated"), "updatedAt");
    }

    #[test]
    fn test_normalize_sort_priority_passthrough() {
        assert_eq!(normalize_sort("priority"), "priority");
    }
}
