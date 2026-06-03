use clap::{Parser, Subcommand};
use std::sync::atomic::{AtomicBool, Ordering};

static MACHINE_MODE: AtomicBool = AtomicBool::new(false);

#[derive(Parser, Debug)]
#[command(
    name = "lql",
    version,
    about = "Query and manage Linear issues from the terminal, in Rust!"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List issues (with filters, sorting, limits)
    List(ListOpts),
    /// Create a new issue
    Create(CreateOpts),
    /// Update an existing issue
    Update(UpdateOpts),
    /// View issue details
    #[command(alias = "show", alias = "get")]
    View(ViewOpts),
    /// Search issues by text
    Search(SearchOpts),
    /// Add a comment to an issue
    Comment(CommentOpts),
    /// List comments on an issue
    Comments(CommentsOpts),
    /// Create or remove a relation between issues
    Relate(RelateOpts),
    /// Remove a relation between issues (shorthand for relate ... unlink)
    Unlink(UnlinkOpts),
    /// Manage labels (list, create, delete)
    Labels(LabelsOpts),
    /// Validate config, auth, teams and labels
    Doctor,
    /// Show resolved context for current directory
    Context,
    /// Manage epics (Linear initiatives with a backing project)
    Epic(EpicOpts),
    /// Manage Linear projects (view, update, comment)
    Project(ProjectOpts),
    /// Execute a raw GraphQL query
    Raw(RawOpts),
}

#[derive(Parser, Debug)]
pub struct ListOpts {
    /// Filter by state (accepts UI names: Todo, Done, "In Progress")
    #[arg(long, alias = "status", value_delimiter = ',')]
    pub state: Option<Vec<String>>,

    /// Filter by label (resolved within the target team when known)
    #[arg(long)]
    pub label: Option<Vec<String>>,

    /// Only issues with no labels
    #[arg(long)]
    pub no_label: bool,

    /// Filter by project (case-insensitive)
    #[arg(long)]
    pub project: Option<String>,

    /// Filter by team (overrides auto-detect)
    #[arg(long)]
    pub team: Option<String>,

    /// Only overdue issues
    #[arg(long)]
    pub overdue: bool,

    /// All teams (ignore context-map)
    #[arg(long)]
    pub all_teams: bool,

    /// Sort by field (default: priority)
    #[arg(long, default_value = "priority")]
    pub sort: String,

    /// Max results
    #[arg(long)]
    pub limit: Option<u32>,

    /// No limit (all results)
    #[arg(long)]
    pub all: bool,

    /// Output as JSONL
    #[arg(long)]
    pub json: bool,

    /// Filter by priority (accepts names: urgent, high, medium, low)
    #[arg(long)]
    pub priority: Option<String>,

    #[arg(long, hide = true)]
    pub no_pager: bool,
    #[arg(long, hide = true)]
    pub no_interactive: bool,
}

#[derive(Parser, Debug)]
pub struct CreateOpts {
    /// Issue title
    pub title: Option<String>,

    /// Issue title (alias for positional — agents prefer named flags)
    #[arg(long = "title", hide = true)]
    pub title_flag: Option<String>,

    /// Inline description
    #[arg(short, long)]
    pub description: Option<String>,

    /// Description from file
    #[arg(long)]
    pub description_file: Option<String>,

    /// Team (overrides auto-detect)
    #[arg(long)]
    pub team: Option<String>,

    /// Project (case-insensitive)
    #[arg(long)]
    pub project: Option<String>,

    /// Label (resolved within the target team)
    #[arg(long)]
    pub label: Option<Vec<String>>,

    /// Priority (name or number)
    #[arg(long)]
    pub priority: Option<String>,

    /// Due date (ISO, relative: friday, +7d)
    #[arg(long)]
    pub due: Option<String>,

    /// Initial state
    #[arg(long, alias = "status")]
    pub state: Option<String>,

    /// Skip duplicate detection
    #[arg(long)]
    pub force: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    #[arg(long, hide = true)]
    pub no_pager: bool,
    #[arg(long, hide = true)]
    pub no_interactive: bool,
}

#[derive(Parser, Debug)]
pub struct UpdateOpts {
    /// Issue ID (e.g. PROD-587)
    pub issue_id: String,

    /// Change state
    #[arg(long, alias = "status")]
    pub state: Option<String>,

    /// Change priority
    #[arg(long)]
    pub priority: Option<String>,

    /// Change project
    #[arg(long)]
    pub project: Option<String>,

    /// Move to a different team
    #[arg(long)]
    pub team: Option<String>,

    /// Add label (resolved within the issue team)
    #[arg(long)]
    pub label: Option<Vec<String>>,

    /// Change title
    #[arg(long)]
    pub title: Option<String>,

    /// Inline description
    #[arg(short, long)]
    pub description: Option<String>,

    /// Description from file
    #[arg(long)]
    pub description_file: Option<String>,

    /// Change due date
    #[arg(long)]
    pub due: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ViewOpts {
    /// Issue ID (e.g. PROD-587)
    pub issue_id: String,

    /// Show comments
    #[arg(long)]
    pub comments: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct SearchOpts {
    /// Search term
    pub query: String,

    /// Filter by team
    #[arg(long)]
    pub team: Option<String>,

    /// Filter by state
    #[arg(long, alias = "status", value_delimiter = ',')]
    pub state: Option<Vec<String>>,

    /// Max results
    #[arg(long)]
    pub limit: Option<u32>,

    /// Output as JSONL
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CommentOpts {
    /// Issue ID (e.g. PROD-587)
    pub issue_id: String,

    /// Comment text
    pub body: Option<String>,

    /// Comment text (alias for positional — agents prefer named flags)
    #[arg(long = "body", hide = true)]
    pub body_flag: Option<String>,

    /// Comment from file
    #[arg(long)]
    pub file: Option<String>,
}

#[derive(Parser, Debug)]
pub struct CommentsOpts {
    /// Issue ID (e.g. PROD-587)
    pub issue_id: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct RelateOpts {
    /// Source issue (e.g. PROD-587)
    pub from: String,

    /// Relation type: blocks, blocked-by, related
    pub relation_type: String,

    /// Target issue (e.g. PROD-588)
    pub to: String,
}

#[derive(Parser, Debug)]
pub struct UnlinkOpts {
    /// First issue (e.g. PROD-587)
    pub from: String,

    /// Second issue (e.g. PROD-588)
    pub to: String,
}

#[derive(Parser, Debug)]
pub struct LabelsOpts {
    #[command(subcommand)]
    pub action: Option<LabelsAction>,

    /// Filter by team (for list)
    #[arg(long)]
    pub team: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum LabelsAction {
    /// List available labels (default)
    List(LabelsListOpts),
    /// Create a new label
    Create(LabelsCreateOpts),
    /// Delete a label
    Delete(LabelsDeleteOpts),
}

#[derive(Parser, Debug)]
pub struct LabelsListOpts {
    /// Filter by team
    #[arg(long)]
    pub team: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct LabelsCreateOpts {
    /// Label name
    pub name: String,

    /// Color (hex, e.g. "#ff0000")
    #[arg(long)]
    pub color: Option<String>,

    /// Assign to team (workspace-level if omitted)
    #[arg(long)]
    pub team: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct LabelsDeleteOpts {
    /// Label name
    pub name: String,
}

#[derive(Parser, Debug)]
pub struct RawOpts {
    /// Inline GraphQL query
    pub query: Option<String>,

    /// Query from file
    #[arg(long)]
    pub file: Option<String>,

    /// Variable key=value (repeatable)
    #[arg(long = "var")]
    pub vars: Option<Vec<String>>,

    /// Variables from JSON file
    #[arg(long)]
    pub vars_file: Option<String>,
}

#[derive(Parser, Debug)]
pub struct EpicOpts {
    #[command(subcommand)]
    pub action: EpicAction,
}

#[derive(Subcommand, Debug)]
pub enum EpicAction {
    /// Create a new epic
    Create(EpicCreateOpts),
    /// List epics
    List(EpicListOpts),
    /// View epic details and issues
    View(EpicViewOpts),
    /// Assign issues to an epic
    Add(EpicAddOpts),
    /// Update an existing epic (title, body, summary, target date)
    Update(EpicUpdateOpts),
    /// Add a comment to an epic (initiative + backing project, if any)
    Comment(EpicCommentOpts),
}

#[derive(Parser, Debug)]
pub struct EpicCreateOpts {
    /// Epic title
    pub title: String,

    /// Inline description
    #[arg(short, long)]
    pub description: Option<String>,

    /// Description from file
    #[arg(long)]
    pub description_file: Option<String>,

    /// Team(s) for the epic backing project (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub team: Option<Vec<String>>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct EpicListOpts {
    /// Filter by team
    #[arg(long)]
    pub team: Option<String>,

    /// Max results
    #[arg(long)]
    pub limit: Option<u32>,

    /// No limit (all results)
    #[arg(long)]
    pub all: bool,

    /// Output as JSONL
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct EpicViewOpts {
    /// Epic ID (slugId, UUID, or Linear URL)
    pub epic_id: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct EpicAddOpts {
    /// Epic ID (slugId, UUID, or Linear URL)
    pub epic_id: String,

    /// Issue IDs to assign
    #[arg(required = true)]
    pub issue_ids: Vec<String>,
}

#[derive(Parser, Debug)]
pub struct EpicUpdateOpts {
    /// Epic ID (slugId, UUID, or Linear URL)
    pub epic_id: String,

    /// Change title (applied to initiative + backing project, truncated)
    #[arg(long)]
    pub title: Option<String>,

    /// Replace the long markdown body (initiative `content` + backing project `content`)
    #[arg(short, long)]
    pub description: Option<String>,

    /// Replace the long markdown body from a file
    #[arg(long)]
    pub description_file: Option<String>,

    /// Update the short initiative summary (Linear `description`, not the long body)
    #[arg(long)]
    pub summary: Option<String>,

    /// Update the initiative target date (YYYY-MM-DD)
    #[arg(long)]
    pub target_date: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct EpicCommentOpts {
    /// Epic ID (slugId, UUID, or Linear URL)
    pub epic_id: String,

    /// Comment text
    pub body: Option<String>,

    /// Comment text (alias for positional — agents prefer named flags)
    #[arg(long = "body", hide = true)]
    pub body_flag: Option<String>,

    /// Comment from file
    #[arg(long)]
    pub file: Option<String>,
}

#[derive(Parser, Debug)]
pub struct ProjectOpts {
    #[command(subcommand)]
    pub action: ProjectAction,
}

#[derive(Subcommand, Debug)]
pub enum ProjectAction {
    /// View project details
    #[command(alias = "show", alias = "get")]
    View(ProjectViewOpts),
    /// Update a project (title, body, summary, target date)
    Update(ProjectUpdateOpts),
    /// Add a comment to a project
    Comment(ProjectCommentOpts),
}

#[derive(Parser, Debug)]
pub struct ProjectViewOpts {
    /// Project ID (UUID, slugId, or name)
    pub project_ref: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ProjectUpdateOpts {
    /// Project ID (UUID, slugId, or name)
    pub project_ref: String,

    /// Change project name
    #[arg(long)]
    pub title: Option<String>,

    /// Replace the long markdown body (project `content`)
    #[arg(short, long)]
    pub description: Option<String>,

    /// Replace the long markdown body from a file
    #[arg(long)]
    pub description_file: Option<String>,

    /// Update the short project summary (Linear `description`, not the long body)
    #[arg(long)]
    pub summary: Option<String>,

    /// Update the project target date (YYYY-MM-DD)
    #[arg(long)]
    pub target_date: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ProjectCommentOpts {
    /// Project ID (UUID, slugId, or name)
    pub project_ref: String,

    /// Comment text
    pub body: Option<String>,

    /// Comment text (alias for positional — agents prefer named flags)
    #[arg(long = "body", hide = true)]
    pub body_flag: Option<String>,

    /// Comment from file
    #[arg(long)]
    pub file: Option<String>,
}

impl CreateOpts {
    pub fn resolved_title(&self) -> Result<&str, String> {
        match (&self.title, &self.title_flag) {
            (Some(t), _) => Ok(t),
            (None, Some(t)) => Ok(t),
            (None, None) => Err(
                "Missing title. Use: lql create \"Title\" or lql create --title \"Title\""
                    .to_string(),
            ),
        }
    }
}

pub fn set_machine_mode(enabled: bool) {
    MACHINE_MODE.store(enabled, Ordering::Relaxed);
}

pub fn machine_mode() -> bool {
    MACHINE_MODE.load(Ordering::Relaxed)
}

pub fn command_prefers_machine_mode(command: &Command) -> bool {
    match command {
        Command::List(opts) => opts.json,
        Command::Create(opts) => opts.json,
        Command::Update(opts) => opts.json,
        Command::View(opts) => opts.json,
        Command::Search(opts) => opts.json,
        Command::Epic(opts) => match &opts.action {
            EpicAction::Create(create) => create.json,
            EpicAction::List(list) => list.json,
            EpicAction::View(view) => view.json,
            EpicAction::Add(_) => false,
            EpicAction::Update(update) => update.json,
            EpicAction::Comment(_) => false,
        },
        Command::Project(opts) => match &opts.action {
            ProjectAction::View(view) => view.json,
            ProjectAction::Update(update) => update.json,
            ProjectAction::Comment(_) => false,
        },
        Command::Labels(opts) => match &opts.action {
            Some(LabelsAction::List(list)) => list.json,
            Some(LabelsAction::Create(create)) => create.json,
            Some(LabelsAction::Delete(_)) => false,
            None => opts.json,
        },
        Command::Comments(opts) => opts.json,
        Command::Comment(_)
        | Command::Relate(_)
        | Command::Unlink(_)
        | Command::Doctor
        | Command::Context
        | Command::Raw(_) => false,
    }
}

fn emit_note(message: &str) {
    if !machine_mode() {
        eprintln!("{message}");
    }
}

/// Normaliza un valor de estado (UI names → API values)
pub fn normalize_state(state: &str, aliases: &std::collections::HashMap<String, String>) -> String {
    // Intentar lookup directo
    if let Some(normalized) = aliases.get(state) {
        emit_note(&format!(
            "ℹ State \"{state}\" → normalized to \"{normalized}\""
        ));
        return normalized.clone();
    }
    // Intentar case-insensitive
    for (key, val) in aliases {
        if key.eq_ignore_ascii_case(state) {
            emit_note(&format!("ℹ State \"{state}\" → normalized to \"{val}\""));
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
        emit_note(&format!("ℹ Priority \"{priority}\" → normalized to {n}"));
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
                emit_note("ℹ Sort \"updated\" → normalized to \"updatedAt\"");
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
        assert_eq!(normalize_state("In Progress", &state_aliases()), "started");
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

    #[test]
    fn test_command_prefers_machine_mode_for_json_commands() {
        let cli = Cli::try_parse_from(["lql", "list", "--json"]).unwrap();
        assert!(command_prefers_machine_mode(&cli.command));

        let cli = Cli::try_parse_from(["lql", "view", "PROD-1", "--json"]).unwrap();
        assert!(command_prefers_machine_mode(&cli.command));

        let cli = Cli::try_parse_from(["lql", "doctor"]).unwrap();
        assert!(!command_prefers_machine_mode(&cli.command));
    }

    #[test]
    fn test_machine_mode_flag_roundtrip() {
        set_machine_mode(true);
        assert!(machine_mode());
        set_machine_mode(false);
        assert!(!machine_mode());
    }

    // --- ERR-11, ERR-12: flags ignorados silenciosamente ---

    #[test]
    fn test_no_pager_accepted_silently() {
        // --no-pager debe ser aceptado sin error por clap
        let result = Cli::try_parse_from(["lql", "list", "--no-pager"]);
        assert!(
            result.is_ok(),
            "ERR-11: --no-pager should be accepted: {result:?}"
        );
    }

    #[test]
    fn test_no_interactive_accepted_silently() {
        let result = Cli::try_parse_from(["lql", "list", "--no-interactive"]);
        assert!(
            result.is_ok(),
            "ERR-12: --no-interactive should be accepted: {result:?}"
        );
    }

    // ERR-02: --status alias funciona
    #[test]
    fn test_status_alias_for_state() {
        let cli = Cli::try_parse_from(["lql", "list", "--status", "Todo"]).unwrap();
        if let Command::List(opts) = cli.command {
            assert_eq!(opts.state.unwrap(), vec!["Todo"]);
        } else {
            panic!("Expected List command");
        }
    }

    // --status en create también funciona
    #[test]
    fn test_status_alias_in_create() {
        let cli = Cli::try_parse_from(["lql", "create", "test", "--status", "Done"]).unwrap();
        if let Command::Create(opts) = cli.command {
            assert_eq!(opts.state.unwrap(), "Done");
        } else {
            panic!("Expected Create command");
        }
    }

    // --status en update
    #[test]
    fn test_status_alias_in_update() {
        let cli = Cli::try_parse_from(["lql", "update", "PROD-1", "--status", "Done"]).unwrap();
        if let Command::Update(opts) = cli.command {
            assert_eq!(opts.state.unwrap(), "Done");
        } else {
            panic!("Expected Update command");
        }
    }

    // Multiple states con coma
    #[test]
    fn test_multiple_states_comma() {
        let cli = Cli::try_parse_from(["lql", "list", "--state", "backlog,unstarted"]).unwrap();
        if let Command::List(opts) = cli.command {
            assert_eq!(opts.state.unwrap(), vec!["backlog", "unstarted"]);
        } else {
            panic!("Expected List command");
        }
    }

    // --- --no-label flag ---

    #[test]
    fn test_no_label_accepted() {
        let result = Cli::try_parse_from(["lql", "list", "--no-label"]);
        assert!(result.is_ok(), "--no-label should be accepted: {result:?}");
        if let Command::List(opts) = result.unwrap().command {
            assert!(opts.no_label);
        } else {
            panic!("Expected List command");
        }
    }

    #[test]
    fn test_no_label_with_label_both_accepted_by_clap() {
        // clap acepta ambos flags; la exclusión mutua se valida en run()
        let result = Cli::try_parse_from(["lql", "list", "--no-label", "--label", "bug"]);
        assert!(result.is_ok(), "clap should accept both flags: {result:?}");
        if let Command::List(opts) = result.unwrap().command {
            assert!(opts.no_label);
            assert!(opts.label.is_some());
        } else {
            panic!("Expected List command");
        }
    }

    // --- ERR-61..64: search CLI parsing ---

    // ERR-61: search acepta query posicional
    #[test]
    fn test_search_parsing() {
        let cli = Cli::try_parse_from(["lql", "search", "basedpyright"]).unwrap();
        if let Command::Search(opts) = cli.command {
            assert_eq!(opts.query, "basedpyright");
        } else {
            panic!("Expected Search");
        }
    }

    // ERR-62: search con --team
    #[test]
    fn test_search_with_team() {
        let cli = Cli::try_parse_from(["lql", "search", "OAuth", "--team", "PROD"]).unwrap();
        if let Command::Search(opts) = cli.command {
            assert_eq!(opts.team.as_deref(), Some("PROD"));
        } else {
            panic!("Expected Search");
        }
    }

    // ERR-63: search con --state
    #[test]
    fn test_search_with_state() {
        let cli = Cli::try_parse_from(["lql", "search", "OAuth", "--state", "backlog,unstarted"])
            .unwrap();
        if let Command::Search(opts) = cli.command {
            assert_eq!(opts.state.unwrap(), vec!["backlog", "unstarted"]);
        } else {
            panic!("Expected Search");
        }
    }

    // --- ERR-65..67: comment CLI parsing ---

    // ERR-65: comment inline
    #[test]
    fn test_comment_inline() {
        let cli = Cli::try_parse_from([
            "lql",
            "comment",
            "PROD-587",
            "Investigado, el problema es X",
        ])
        .unwrap();
        if let Command::Comment(opts) = cli.command {
            assert_eq!(opts.issue_id, "PROD-587");
            assert_eq!(opts.body.as_deref(), Some("Investigado, el problema es X"));
        } else {
            panic!("Expected Comment");
        }
    }

    // ERR-66: comment desde fichero
    #[test]
    fn test_comment_from_file() {
        let cli =
            Cli::try_parse_from(["lql", "comment", "PROD-587", "--file", "/tmp/c.md"]).unwrap();
        if let Command::Comment(opts) = cli.command {
            assert!(opts.body.is_none());
            assert_eq!(opts.file.as_deref(), Some("/tmp/c.md"));
        } else {
            panic!("Expected Comment");
        }
    }

    // --- ERR-68..70: relate CLI parsing ---

    // ERR-68: relate blocks
    #[test]
    fn test_relate_blocks_parsing() {
        let cli = Cli::try_parse_from(["lql", "relate", "PROD-587", "blocks", "PROD-588"]).unwrap();
        if let Command::Relate(opts) = cli.command {
            assert_eq!(opts.from, "PROD-587");
            assert_eq!(opts.relation_type, "blocks");
            assert_eq!(opts.to, "PROD-588");
        } else {
            panic!("Expected Relate");
        }
    }

    // ERR-69: relate blocked-by
    #[test]
    fn test_relate_blocked_by_parsing() {
        let cli =
            Cli::try_parse_from(["lql", "relate", "PROD-587", "blocked-by", "PROD-515"]).unwrap();
        if let Command::Relate(opts) = cli.command {
            assert_eq!(opts.relation_type, "blocked-by");
        } else {
            panic!("Expected Relate");
        }
    }

    // ERR-70: relate related
    #[test]
    fn test_relate_related_parsing() {
        let cli =
            Cli::try_parse_from(["lql", "relate", "PROD-587", "related", "PROD-520"]).unwrap();
        if let Command::Relate(opts) = cli.command {
            assert_eq!(opts.relation_type, "related");
        } else {
            panic!("Expected Relate");
        }
    }

    // relate unlink via relate subcommand
    #[test]
    fn test_relate_unlink_parsing() {
        let cli = Cli::try_parse_from(["lql", "relate", "PROD-587", "unlink", "PROD-588"]).unwrap();
        if let Command::Relate(opts) = cli.command {
            assert_eq!(opts.relation_type, "unlink");
        } else {
            panic!("Expected Relate");
        }
    }

    // unlink como comando directo
    #[test]
    fn test_unlink_command_parsing() {
        let cli = Cli::try_parse_from(["lql", "unlink", "PROD-587", "PROD-588"]).unwrap();
        if let Command::Unlink(opts) = cli.command {
            assert_eq!(opts.from, "PROD-587");
            assert_eq!(opts.to, "PROD-588");
        } else {
            panic!("Expected Unlink");
        }
    }

    // Priority case-insensitive
    #[test]
    fn test_normalize_priority_case_insensitive() {
        assert_eq!(normalize_priority("Urgent", &priority_aliases()), Ok(1));
        assert_eq!(normalize_priority("HIGH", &priority_aliases()), Ok(2));
        assert_eq!(normalize_priority("MEDIUM", &priority_aliases()), Ok(3));
    }

    // --- Labels subcommands ---

    // labels sin subcomando = list (backward compatible)
    #[test]
    fn test_labels_no_subcommand() {
        let cli = Cli::try_parse_from(["lql", "labels"]).unwrap();
        if let Command::Labels(opts) = cli.command {
            assert!(opts.action.is_none());
        } else {
            panic!("Expected Labels command");
        }
    }

    // labels list explícito
    #[test]
    fn test_labels_list_subcommand() {
        let cli = Cli::try_parse_from(["lql", "labels", "list"]).unwrap();
        if let Command::Labels(opts) = cli.command {
            assert!(matches!(opts.action, Some(LabelsAction::List(_))));
        } else {
            panic!("Expected Labels command");
        }
    }

    // labels list --json
    #[test]
    fn test_labels_list_json() {
        let cli = Cli::try_parse_from(["lql", "labels", "list", "--json"]).unwrap();
        if let Command::Labels(opts) = cli.command {
            if let Some(LabelsAction::List(list_opts)) = opts.action {
                assert!(list_opts.json);
            } else {
                panic!("Expected List subcommand");
            }
        } else {
            panic!("Expected Labels command");
        }
    }

    // labels --json (en root, backward compatible)
    #[test]
    fn test_labels_root_json_flag() {
        let cli = Cli::try_parse_from(["lql", "labels", "--json"]).unwrap();
        if let Command::Labels(opts) = cli.command {
            assert!(opts.json);
        } else {
            panic!("Expected Labels command");
        }
    }

    // labels create
    #[test]
    fn test_labels_create() {
        let cli = Cli::try_parse_from(["lql", "labels", "create", "bug"]).unwrap();
        if let Command::Labels(opts) = cli.command {
            if let Some(LabelsAction::Create(create_opts)) = opts.action {
                assert_eq!(create_opts.name, "bug");
                assert!(create_opts.color.is_none());
                assert!(create_opts.team.is_none());
            } else {
                panic!("Expected Create subcommand");
            }
        } else {
            panic!("Expected Labels command");
        }
    }

    // labels create con --color y --team
    #[test]
    fn test_labels_create_with_options() {
        let cli = Cli::try_parse_from([
            "lql", "labels", "create", "bug", "--color", "#ff0000", "--team", "PROD",
        ])
        .unwrap();
        if let Command::Labels(opts) = cli.command {
            if let Some(LabelsAction::Create(create_opts)) = opts.action {
                assert_eq!(create_opts.name, "bug");
                assert_eq!(create_opts.color.as_deref(), Some("#ff0000"));
                assert_eq!(create_opts.team.as_deref(), Some("PROD"));
            } else {
                panic!("Expected Create subcommand");
            }
        } else {
            panic!("Expected Labels command");
        }
    }

    // labels delete
    #[test]
    fn test_labels_delete() {
        let cli = Cli::try_parse_from(["lql", "labels", "delete", "bug"]).unwrap();
        if let Command::Labels(opts) = cli.command {
            if let Some(LabelsAction::Delete(delete_opts)) = opts.action {
                assert_eq!(delete_opts.name, "bug");
            } else {
                panic!("Expected Delete subcommand");
            }
        } else {
            panic!("Expected Labels command");
        }
    }

    // ERR-13 variantes
    #[test]
    fn test_normalize_sort_variants() {
        assert_eq!(normalize_sort("Updated"), "updatedAt");
        assert_eq!(normalize_sort("UPDATED"), "updatedAt");
        assert_eq!(normalize_sort("updatedAt"), "updatedAt");
        assert_eq!(normalize_sort("created"), "createdAt");
        assert_eq!(normalize_sort("createdAt"), "createdAt");
    }

    // ===================================================================
    // Agentic experience tests — fixtures from real Claude Code sessions.
    // Each test reproduces a real invocation that failed in production.
    // ===================================================================

    // --- AX-01: `lql show PROD-911` → should parse as View ---
    // Real: "error: unrecognized subcommand 'show'"
    #[test]
    fn test_show_alias_for_view() {
        let cli = Cli::try_parse_from(["lql", "show", "PROD-911"]).unwrap();
        if let Command::View(opts) = cli.command {
            assert_eq!(opts.issue_id, "PROD-911");
        } else {
            panic!("Expected View command from 'show' alias");
        }
    }

    // --- AX-02: `lql get PROD-86` → should parse as View ---
    // Real: "error: unrecognized subcommand 'get'"
    #[test]
    fn test_get_alias_for_view() {
        let cli = Cli::try_parse_from(["lql", "get", "PROD-86"]).unwrap();
        if let Command::View(opts) = cli.command {
            assert_eq!(opts.issue_id, "PROD-86");
        } else {
            panic!("Expected View command from 'get' alias");
        }
    }

    // --- AX-03: `lql show PROD-911 --json` → should work with flags ---
    #[test]
    fn test_show_alias_with_json() {
        let cli = Cli::try_parse_from(["lql", "show", "PROD-911", "--json"]).unwrap();
        if let Command::View(opts) = cli.command {
            assert!(opts.json);
        } else {
            panic!("Expected View command from 'show' alias");
        }
    }

    // --- AX-04: `lql create --title "Epic: ..." --team PROD` ---
    // Real: "error: unexpected argument '--title' found"
    #[test]
    fn test_create_title_as_flag() {
        let cli = Cli::try_parse_from([
            "lql",
            "create",
            "--title",
            "Epic: Pre-locale — preparar ETL para multi-locale sin romper ES",
            "--team",
            "PROD",
        ])
        .unwrap();
        if let Command::Create(opts) = cli.command {
            assert_eq!(
                opts.resolved_title().unwrap(),
                "Epic: Pre-locale — preparar ETL para multi-locale sin romper ES"
            );
            assert_eq!(opts.team.as_deref(), Some("PROD"));
        } else {
            panic!("Expected Create command");
        }
    }

    // --- AX-05: `lql create --team QIN --title "Migrar names.db"` ---
    // Real: same --title error, different team position
    #[test]
    fn test_create_title_flag_before_team() {
        let cli = Cli::try_parse_from([
            "lql",
            "create",
            "--team",
            "QIN",
            "--title",
            "Migrar names.db y artefactos v1 a data/",
        ])
        .unwrap();
        if let Command::Create(opts) = cli.command {
            assert_eq!(
                opts.resolved_title().unwrap(),
                "Migrar names.db y artefactos v1 a data/"
            );
        } else {
            panic!("Expected Create command");
        }
    }

    // AX-04b: positional title still works
    #[test]
    fn test_create_positional_title_still_works() {
        let cli = Cli::try_parse_from(["lql", "create", "My title"]).unwrap();
        if let Command::Create(opts) = cli.command {
            assert_eq!(opts.resolved_title().unwrap(), "My title");
        } else {
            panic!("Expected Create command");
        }
    }

    // AX-04c: no title at all → error
    #[test]
    fn test_create_no_title_error() {
        let cli = Cli::try_parse_from(["lql", "create", "--team", "PROD"]).unwrap();
        if let Command::Create(opts) = cli.command {
            assert!(opts.resolved_title().is_err());
        } else {
            panic!("Expected Create command");
        }
    }

    // --- AX-06: `lql update PRIV-32 --team PROD` → move issue ---
    // Real: "error: unexpected argument '--team' found"
    #[test]
    fn test_update_team_flag() {
        let cli = Cli::try_parse_from(["lql", "update", "PRIV-32", "--team", "PROD"]).unwrap();
        if let Command::Update(opts) = cli.command {
            assert_eq!(opts.issue_id, "PRIV-32");
            assert_eq!(opts.team.as_deref(), Some("PROD"));
        } else {
            panic!("Expected Update command with --team");
        }
    }

    // --- AX-07: `lql view PROD-824 --comments` → show comments ---
    // Real: "error: unexpected argument '--comments' found"
    #[test]
    fn test_view_comments_flag() {
        let cli = Cli::try_parse_from(["lql", "view", "PROD-824", "--comments"]).unwrap();
        if let Command::View(opts) = cli.command {
            assert!(opts.comments);
            assert_eq!(opts.issue_id, "PROD-824");
        } else {
            panic!("Expected View command with --comments");
        }
    }

    // --- AX-09: `lql comments PROD-975` → should parse as Comments ---
    // Real: "error: unrecognized subcommand 'comments'"
    #[test]
    fn test_comments_subcommand() {
        let cli = Cli::try_parse_from(["lql", "comments", "PROD-975"]).unwrap();
        if let Command::Comments(opts) = cli.command {
            assert_eq!(opts.issue_id, "PROD-975");
            assert!(!opts.json);
        } else {
            panic!("Expected Comments command");
        }
    }

    // AX-09b: `lql comments PROD-975 --json` → should work with --json
    #[test]
    fn test_comments_subcommand_json() {
        let cli = Cli::try_parse_from(["lql", "comments", "PROD-975", "--json"]).unwrap();
        if let Command::Comments(opts) = cli.command {
            assert_eq!(opts.issue_id, "PROD-975");
            assert!(opts.json);
        } else {
            panic!("Expected Comments command with --json");
        }
    }

    // --- AX-08: `lql comment PROD-926 --body "text"` → body as flag ---
    // Real: "error: unexpected argument '--body' found"
    #[test]
    fn test_comment_body_as_flag() {
        let cli = Cli::try_parse_from([
            "lql",
            "comment",
            "PROD-926",
            "--body",
            "Investigado, el problema es X",
        ])
        .unwrap();
        if let Command::Comment(opts) = cli.command {
            assert_eq!(
                opts.body_flag.as_deref(),
                Some("Investigado, el problema es X")
            );
            assert!(opts.body.is_none());
        } else {
            panic!("Expected Comment command");
        }
    }

    // ===================================================================
    // `lql epic update` / `lql epic comment` / `lql project ...` parsing.
    // Acceptance tests from docs/epic-update-contract.md.
    // ===================================================================

    #[test]
    fn test_epic_update_accepts_description_file() {
        let cli = Cli::try_parse_from([
            "lql",
            "epic",
            "update",
            "cb19ff35fa52",
            "--description-file",
            "plan.md",
        ])
        .unwrap();
        if let Command::Epic(opts) = cli.command {
            if let EpicAction::Update(update) = opts.action {
                assert_eq!(update.epic_id, "cb19ff35fa52");
                assert_eq!(update.description_file.as_deref(), Some("plan.md"));
            } else {
                panic!("Expected EpicAction::Update");
            }
        } else {
            panic!("Expected Epic command");
        }
    }

    #[test]
    fn test_epic_update_accepts_target_date() {
        let cli = Cli::try_parse_from([
            "lql",
            "epic",
            "update",
            "cb19ff35fa52",
            "--title",
            "New title",
            "--summary",
            "Short",
            "--target-date",
            "2026-06-15",
        ])
        .unwrap();
        if let Command::Epic(opts) = cli.command {
            if let EpicAction::Update(update) = opts.action {
                assert_eq!(update.title.as_deref(), Some("New title"));
                assert_eq!(update.summary.as_deref(), Some("Short"));
                assert_eq!(update.target_date.as_deref(), Some("2026-06-15"));
            } else {
                panic!("Expected EpicAction::Update");
            }
        } else {
            panic!("Expected Epic command");
        }
    }

    #[test]
    fn test_epic_comment_inline_body() {
        let cli =
            Cli::try_parse_from(["lql", "epic", "comment", "cb19ff35fa52", "Progress update"])
                .unwrap();
        if let Command::Epic(opts) = cli.command {
            if let EpicAction::Comment(comment) = opts.action {
                assert_eq!(comment.epic_id, "cb19ff35fa52");
                assert_eq!(comment.body.as_deref(), Some("Progress update"));
                assert!(comment.body_flag.is_none());
            } else {
                panic!("Expected EpicAction::Comment");
            }
        } else {
            panic!("Expected Epic command");
        }
    }

    #[test]
    fn test_epic_comment_from_file() {
        let cli = Cli::try_parse_from([
            "lql",
            "epic",
            "comment",
            "cb19ff35fa52",
            "--file",
            "/tmp/c.md",
        ])
        .unwrap();
        if let Command::Epic(opts) = cli.command {
            if let EpicAction::Comment(comment) = opts.action {
                assert!(comment.body.is_none());
                assert_eq!(comment.file.as_deref(), Some("/tmp/c.md"));
            } else {
                panic!("Expected EpicAction::Comment");
            }
        } else {
            panic!("Expected Epic command");
        }
    }

    #[test]
    fn test_project_view_parses() {
        let cli = Cli::try_parse_from(["lql", "project", "view", "Bastidor v1.0"]).unwrap();
        if let Command::Project(opts) = cli.command {
            if let ProjectAction::View(view) = opts.action {
                assert_eq!(view.project_ref, "Bastidor v1.0");
                assert!(!view.json);
            } else {
                panic!("Expected ProjectAction::View");
            }
        } else {
            panic!("Expected Project command");
        }
    }

    #[test]
    fn test_project_view_aliases() {
        // `show` and `get` should both alias `view` for parity with `lql view`.
        for alias in ["show", "get"] {
            let cli = Cli::try_parse_from(["lql", "project", alias, "some-slug"]).unwrap();
            if let Command::Project(opts) = cli.command {
                assert!(
                    matches!(opts.action, ProjectAction::View(_)),
                    "alias {alias}"
                );
            } else {
                panic!("Expected Project command for alias {alias}");
            }
        }
    }

    #[test]
    fn test_project_update_parses() {
        let cli = Cli::try_parse_from([
            "lql",
            "project",
            "update",
            "some-slug",
            "--description-file",
            "plan.md",
            "--target-date",
            "2026-06-15",
        ])
        .unwrap();
        if let Command::Project(opts) = cli.command {
            if let ProjectAction::Update(update) = opts.action {
                assert_eq!(update.project_ref, "some-slug");
                assert_eq!(update.description_file.as_deref(), Some("plan.md"));
                assert_eq!(update.target_date.as_deref(), Some("2026-06-15"));
            } else {
                panic!("Expected ProjectAction::Update");
            }
        } else {
            panic!("Expected Project command");
        }
    }

    #[test]
    fn test_project_comment_parses() {
        let cli = Cli::try_parse_from(["lql", "project", "comment", "some-slug", "Progress note"])
            .unwrap();
        if let Command::Project(opts) = cli.command {
            if let ProjectAction::Comment(comment) = opts.action {
                assert_eq!(comment.project_ref, "some-slug");
                assert_eq!(comment.body.as_deref(), Some("Progress note"));
            } else {
                panic!("Expected ProjectAction::Comment");
            }
        } else {
            panic!("Expected Project command");
        }
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;
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

    // Cualquier casing de "Todo" normaliza a "unstarted"
    proptest! {
        #[test]
        fn prop_todo_any_case_normalizes(
            s in prop::string::string_regex("[tT][oO][dD][oO]").unwrap()
        ) {
            let result = normalize_state(&s, &state_aliases());
            prop_assert_eq!(result, "unstarted");
        }

        #[test]
        fn prop_done_any_case_normalizes(
            s in prop::string::string_regex("[dD][oO][nN][eE]").unwrap()
        ) {
            let result = normalize_state(&s, &state_aliases());
            prop_assert_eq!(result, "completed");
        }

        // Cualquier casing de priority names normaliza al número correcto
        #[test]
        fn prop_urgent_any_case(
            s in prop::string::string_regex("[uU][rR][gG][eE][nN][tT]").unwrap()
        ) {
            let result = normalize_priority(&s, &priority_aliases());
            prop_assert_eq!(result, Ok(1));
        }

        #[test]
        fn prop_high_any_case(
            s in prop::string::string_regex("[hH][iI][gG][hH]").unwrap()
        ) {
            let result = normalize_priority(&s, &priority_aliases());
            prop_assert_eq!(result, Ok(2));
        }

        // Valores API válidos pasan sin cambio
        #[test]
        fn prop_api_states_passthrough(
            s in prop::sample::select(vec![
                "backlog".to_string(),
                "unstarted".to_string(),
                "started".to_string(),
                "completed".to_string(),
                "canceled".to_string(),
            ])
        ) {
            let result = normalize_state(&s, &state_aliases());
            prop_assert_eq!(result, s);
        }

        // Priority números 0-4 son válidos
        #[test]
        fn prop_priority_valid_numbers(n in 0u8..=4) {
            let result = normalize_priority(&n.to_string(), &priority_aliases());
            prop_assert_eq!(result, Ok(n));
        }

        // Priority números >4 son inválidos
        #[test]
        fn prop_priority_invalid_numbers(n in 5u8..=255) {
            let result = normalize_priority(&n.to_string(), &priority_aliases());
            prop_assert!(result.is_err());
        }

        // Sort normaliza "updated" en cualquier casing a "updatedAt"
        #[test]
        fn prop_sort_updated_any_case(
            s in prop::string::string_regex("[uU][pP][dD][aA][tT][eE][dD]").unwrap()
        ) {
            let result = normalize_sort(&s);
            prop_assert_eq!(result, "updatedAt");
        }
    }
}
