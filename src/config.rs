use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default, rename = "context-map")]
    pub context_map: HashMap<String, ContextEntry>,
    #[serde(default, rename = "state-aliases")]
    pub state_aliases: HashMap<String, String>,
    #[serde(default, rename = "priority-aliases")]
    pub priority_aliases: HashMap<String, u8>,
    #[serde(default, rename = "retired-teams")]
    pub retired_teams: HashMap<String, String>,
}

/// Authentication configuration.
///
/// Credential resolution order (first match wins):
///   1. `LINEAR_API_KEY` env var
///   2. `[auth].command`  — arbitrary credential helper (e.g. `["pass", "show", "linear"]`)
///   3. `[auth].api_key_ref` — sugar for `["op", "read", "<ref>"]` (1Password CLI)
///
/// With none set, the user gets a guided error message.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct AuthConfig {
    #[serde(default)]
    pub api_key_ref: Option<String>,
    #[serde(default)]
    pub command: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Defaults {
    #[serde(default = "default_sort")]
    #[allow(dead_code)]
    pub sort: String,
    #[serde(default = "default_states")]
    pub states: Vec<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Fallback team used when no `--team` is given and the cwd matches no
    /// `[context-map]` entry. When it kicks in, callers announce the
    /// substitution on stderr (see `team_fallback_warning`).
    #[serde(default)]
    pub team: Option<String>,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            sort: default_sort(),
            states: default_states(),
            limit: default_limit(),
            team: None,
        }
    }
}

/// How `resolve_team` arrived at the team it returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamSource {
    /// An explicit `--team` override.
    Override,
    /// A `[context-map]` entry matched the cwd.
    Context,
    /// The `[defaults] team` fallback (no override, no context match).
    Default,
}

/// Message announcing that no team was detected and the configured default was
/// substituted. Callers emit it to stderr — never stdout, which carries the
/// TOON/machine payload (semantic honesty: announce the implicit fallback).
pub fn team_fallback_warning(team: &str) -> String {
    format!(
        "no team detected from context; using configured default team {team} ([defaults].team)"
    )
}

fn default_sort() -> String {
    "priority".to_string()
}
fn default_states() -> Vec<String> {
    vec![
        "backlog".to_string(),
        "unstarted".to_string(),
        "started".to_string(),
    ]
}
fn default_limit() -> u32 {
    50
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContextEntry {
    pub team: String,
    pub project: Option<String>,
    pub label: Option<String>,
}

/// Contexto resuelto del directorio actual
#[derive(Debug, Clone)]
pub struct ResolvedContext {
    pub team: String,
    pub project: Option<String>,
    pub label: Option<String>,
    pub source: String,
}

impl Config {
    /// Resuelve el contexto del cwd contra el context-map
    pub fn resolve_context(&self, cwd: &Path) -> Option<ResolvedContext> {
        let cwd_str = cwd.to_string_lossy();

        // Buscar match más largo (más específico)
        let mut best_match: Option<(&str, &ContextEntry)> = None;
        for (pattern, entry) in &self.context_map {
            let expanded = expand_tilde(pattern);
            if cwd_str.starts_with(&expanded) {
                match best_match {
                    None => best_match = Some((pattern, entry)),
                    Some((prev_pattern, _)) => {
                        if expanded.len() > expand_tilde(prev_pattern).len() {
                            best_match = Some((pattern, entry));
                        }
                    }
                }
            }
        }

        best_match.map(|(pattern, entry)| ResolvedContext {
            team: entry.team.clone(),
            project: entry.project.clone(),
            label: entry.label.clone(),
            source: format!("{pattern} in {}", config_path().display()),
        })
    }

    /// Resuelve el team: override > context-map > default > error.
    ///
    /// El cuarto elemento indica la procedencia (`TeamSource`) para que el caller
    /// pueda anunciar por stderr cuando se ha usado el default (honestidad
    /// semántica: no aplicar un fallback implícito en silencio).
    pub fn resolve_team(
        &self,
        team_override: Option<&str>,
        cwd: &Path,
    ) -> Result<(String, Option<String>, Option<String>, TeamSource), String> {
        if let Some(team) = team_override {
            // Comprobar teams retirados (case-insensitive, ley de Postel).
            if let Some(msg) = self.retired_team_message(team) {
                return Err(format!("Team {team} is retired. {msg}"));
            }
            return Ok((team.to_string(), None, None, TeamSource::Override));
        }

        if let Some(ctx) = self.resolve_context(cwd) {
            // A retired team must never resolve as live, whatever the path it
            // came from — including a stale [context-map] entry.
            if let Some(msg) = self.retired_team_message(&ctx.team) {
                return Err(format!("Team {} is retired. {msg}", ctx.team));
            }
            return Ok((ctx.team, ctx.project, ctx.label, TeamSource::Context));
        }

        if let Some(default_team) = self.defaults.team.as_deref() {
            // Un default retirado se rechaza igual que un --team retirado: el
            // fallback no puede colar un team muerto.
            if let Some(msg) = self.retired_team_message(default_team) {
                return Err(format!("Team {default_team} is retired. {msg}"));
            }
            return Ok((
                default_team.to_string(),
                None,
                None,
                TeamSource::Default,
            ));
        }

        let cwd_display = cwd.display();
        Err(format!(
            "Could not detect team from {cwd_display}. Use --team <TEAM> or add a [context-map] entry in {}. See: lql doctor",
            config_path().display()
        ))
    }

    /// Looks up the retirement message for a team key **case-insensitively**
    /// (Postel's law: `tok`, `TOK` and `Tok` must all trip the retired hint).
    pub(crate) fn retired_team_message(&self, team: &str) -> Option<&str> {
        self.retired_teams
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(team))
            .map(|(_, msg)| msg.as_str())
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return format!("{}/{rest}", home.display());
    }
    path.to_string()
}

pub fn config_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".config/lql/config.toml");
    path
}

pub fn load() -> Result<Config, String> {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            toml::from_str(&content).map_err(|e| format!("Invalid config {}: {e}", path.display()))
        }
        // Missing config is OK: defaults work as long as LINEAR_API_KEY is set.
        // Surface I/O errors other than NotFound so permission/format issues stay visible.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(format!("Could not read {}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        let toml_str = r#"
[auth]
api_key_ref = "op://Personal/Linear/api-key"

[defaults]
sort = "priority"
states = ["backlog", "unstarted", "started"]
limit = 50

[context-map]
"~/code/reactor" = { team = "PROD", project = "Reactor", label = "reactor" }
"~/code/phoenix" = { team = "PROD", project = "Phoenix", label = "phoenix" }
"~/code/blog" = { team = "CONT", project = "Blog", label = "blog" }
"~/code/homelab" = { team = "PRIV", label = "homelab" }
"~/code/lql" = { team = "TOOL", label = "lql" }

[state-aliases]
"Todo" = "unstarted"
"In Progress" = "started"
"Done" = "completed"
"Canceled" = "canceled"
"Cancelled" = "canceled"

[priority-aliases]
urgent = 1
high = 2
medium = 3
low = 4
none = 0

[retired-teams]
TOK = "Reactor issues are now in PROD. Use: --team PROD --label reactor"
QIN = "Use: --team PROD --label phoenix"
"#;
        toml::from_str(toml_str).unwrap()
    }

    // ERR-19: auto-detect team desde cwd
    #[test]
    fn test_resolve_context_reactor() {
        let config = test_config();
        let home = dirs::home_dir().unwrap();
        let cwd = home.join("code/reactor");
        let ctx = config.resolve_context(&cwd).unwrap();
        assert_eq!(ctx.team, "PROD");
        assert_eq!(ctx.project.as_deref(), Some("Reactor"));
        assert_eq!(ctx.label.as_deref(), Some("reactor"));
    }

    #[test]
    fn test_resolve_context_phoenix() {
        let config = test_config();
        let home = dirs::home_dir().unwrap();
        let cwd = home.join("code/phoenix");
        let ctx = config.resolve_context(&cwd).unwrap();
        assert_eq!(ctx.team, "PROD");
        assert_eq!(ctx.label.as_deref(), Some("phoenix"));
    }

    // ERR-21: cwd sin match
    #[test]
    fn test_resolve_context_no_match() {
        let config = test_config();
        let cwd = Path::new("/tmp");
        assert!(config.resolve_context(cwd).is_none());
    }

    // ERR-22: --team override
    #[test]
    fn test_resolve_team_override() {
        let config = test_config();
        let home = dirs::home_dir().unwrap();
        let cwd = home.join("code/reactor");
        let (team, project, label, source) = config.resolve_team(Some("CONT"), &cwd).unwrap();
        assert_eq!(team, "CONT");
        assert!(project.is_none()); // override no trae project/label
        assert!(label.is_none());
        assert_eq!(source, TeamSource::Override);
    }

    // T01: default team fallback (no override, no context match)
    #[test]
    fn test_resolve_team_falls_back_to_default() {
        let mut config = test_config();
        config.defaults.team = Some("KC".to_string());
        let cwd = Path::new("/tmp/not-a-linked-repo");
        let (team, project, label, source) = config.resolve_team(None, cwd).unwrap();
        assert_eq!(team, "KC");
        assert!(project.is_none());
        assert!(label.is_none());
        assert_eq!(source, TeamSource::Default);
    }

    // T01: an explicit override beats the configured default.
    #[test]
    fn test_resolve_team_override_beats_default() {
        let mut config = test_config();
        config.defaults.team = Some("KC".to_string());
        let cwd = Path::new("/tmp/not-a-linked-repo");
        let (team, _, _, source) = config.resolve_team(Some("CONT"), cwd).unwrap();
        assert_eq!(team, "CONT");
        assert_eq!(source, TeamSource::Override);
    }

    // T01: a context-map match beats the configured default.
    #[test]
    fn test_resolve_team_context_map_beats_default() {
        let mut config = test_config();
        config.defaults.team = Some("KC".to_string());
        let home = dirs::home_dir().unwrap();
        let cwd = home.join("code/reactor");
        let (team, _, _, source) = config.resolve_team(None, &cwd).unwrap();
        assert_eq!(team, "PROD"); // from context-map, not the default
        assert_eq!(source, TeamSource::Context);
    }

    // T01: a default that names a retired team is rejected, same as an explicit
    // --team — the fallback must not smuggle a retired team past the check.
    #[test]
    fn test_resolve_team_default_retired_is_rejected() {
        let mut config = test_config();
        config.defaults.team = Some("TOK".to_string()); // retired in test_config
        let cwd = Path::new("/tmp/not-a-linked-repo");
        let result = config.resolve_team(None, cwd);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("retired"));
    }

    // T01: with no default and no match, the original error is unchanged.
    #[test]
    fn test_resolve_team_no_default_no_match_errors_unchanged() {
        let config = test_config(); // defaults.team is None
        let cwd = Path::new("/tmp/not-a-linked-repo");
        let err = config.resolve_team(None, cwd).unwrap_err();
        assert!(err.starts_with("Could not detect team from /tmp/not-a-linked-repo"));
        assert!(err.contains("Use --team <TEAM>"));
        assert!(err.contains("See: lql doctor"));
    }

    // T01: the fallback warning names the substituted team and points at config.
    #[test]
    fn test_team_fallback_warning_names_team() {
        let msg = team_fallback_warning("KC");
        assert!(msg.contains("KC"), "{msg}");
        assert!(msg.contains("default"), "{msg}");
        assert!(msg.contains("[defaults].team"), "{msg}");
    }

    // ERR-34: team retirado TOK
    #[test]
    fn test_retired_team_tok() {
        let config = test_config();
        let cwd = Path::new("/tmp");
        let result = config.resolve_team(Some("TOK"), cwd);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("retired"));
    }

    // ERR-35: team retirado QIN
    #[test]
    fn test_retired_team_qin() {
        let config = test_config();
        let cwd = Path::new("/tmp");
        let result = config.resolve_team(Some("QIN"), cwd);
        assert!(result.is_err());
    }

    // FIX A: retired check is case-insensitive on the --team override path
    // (Postel's law: accept tok/Tok/tOk and still trip the retired hint).
    #[test]
    fn test_retired_team_override_case_insensitive() {
        let config = test_config();
        let cwd = Path::new("/tmp");
        for variant in ["tok", "Tok", "tOk", "qin", "Qin"] {
            let err = config
                .resolve_team(Some(variant), cwd)
                .unwrap_err();
            assert!(err.contains("retired"), "{variant} -> {err}");
        }
    }

    // FIX A: a retired team reached via a stale [context-map] entry must also be
    // rejected — a retired team must never resolve as live, whatever the path.
    #[test]
    fn test_resolve_team_context_map_retired_is_rejected() {
        let mut config = test_config();
        let home = dirs::home_dir().unwrap();
        let dir = home.join("code/legacy-tok");
        config.context_map.insert(
            dir.to_string_lossy().into_owned(),
            ContextEntry {
                team: "TOK".to_string(),
                project: None,
                label: None,
            },
        );
        let err = config.resolve_team(None, &dir).unwrap_err();
        assert!(err.contains("retired"), "{err}");
    }

    // FIX A: retired check is case-insensitive on the [defaults] team path too.
    #[test]
    fn test_resolve_team_default_retired_case_insensitive() {
        let cwd = Path::new("/tmp/not-a-linked-repo");
        for variant in ["tok", "Tok", "qin"] {
            let mut config = test_config();
            config.defaults.team = Some(variant.to_string());
            let err = config.resolve_team(None, cwd).unwrap_err();
            assert!(err.contains("retired"), "default {variant} -> {err}");
        }
    }

    // ERR-01: default sort es priority
    #[test]
    fn test_defaults_sort_priority() {
        let defaults = Defaults::default();
        assert_eq!(defaults.sort, "priority");
    }

    // ERR-01b: default states son backlog, unstarted, started
    #[test]
    fn test_defaults_states() {
        let defaults = Defaults::default();
        assert_eq!(defaults.states, vec!["backlog", "unstarted", "started"]);
    }

    // ERR-01c: default limit es 50
    #[test]
    fn test_defaults_limit() {
        let defaults = Defaults::default();
        assert_eq!(defaults.limit, 50);
    }

    // Subdirectorio también matchea
    #[test]
    fn test_resolve_context_subdirectory() {
        let config = test_config();
        let home = dirs::home_dir().unwrap();
        let cwd = home.join("code/reactor/src/deep/nested");
        let ctx = config.resolve_context(&cwd).unwrap();
        assert_eq!(ctx.team, "PROD");
        assert_eq!(ctx.label.as_deref(), Some("reactor"));
    }
}
