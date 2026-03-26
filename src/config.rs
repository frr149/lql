use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
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

#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    pub api_key_ref: String,
}

#[derive(Debug, Deserialize)]
pub struct Defaults {
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_states")]
    pub states: Vec<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            sort: default_sort(),
            states: default_states(),
            limit: default_limit(),
        }
    }
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

    /// Resuelve el team: override > context-map > error
    pub fn resolve_team(
        &self,
        team_override: Option<&str>,
        cwd: &Path,
    ) -> Result<(String, Option<String>, Option<String>), String> {
        if let Some(team) = team_override {
            // Comprobar teams retirados
            if let Some(msg) = self.retired_teams.get(team) {
                return Err(format!("Team {team} is retired. {msg}"));
            }
            return Ok((team.to_string(), None, None));
        }

        match self.resolve_context(cwd) {
            Some(ctx) => Ok((ctx.team, ctx.project, ctx.label)),
            None => {
                let cwd_display = cwd.display();
                Err(format!(
                    "Could not detect team from {cwd_display}. Use --team <TEAM>. Available: PROD, CONT, PRIV, TOOL, KC"
                ))
            }
        }
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{rest}", home.display());
        }
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
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let config: Config =
        toml::from_str(&content).map_err(|e| format!("Invalid config {}: {e}", path.display()))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        let toml_str = r#"
[auth]
api_key_ref = "op://Private/Linear/api-key"

[defaults]
sort = "priority"
states = ["backlog", "unstarted", "started"]
limit = 50

[context-map]
"~/projects/tokamak" = { team = "PROD", project = "Tokamak", label = "tokamak" }
"~/code/acme" = { team = "PROD", project = "Acme", label = "acme" }
"~/code/frr.dev" = { team = "CONT", project = "Blog", label = "blog" }
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
TOK = "Tokamak issues are now in PROD. Use: --team PROD --label tokamak"
QIN = "Use: --team PROD --label acme"
"#;
        toml::from_str(toml_str).unwrap()
    }

    // ERR-19: auto-detect team desde cwd
    #[test]
    fn test_resolve_context_tokamak() {
        let config = test_config();
        let home = dirs::home_dir().unwrap();
        let cwd = home.join("code/tokamak");
        let ctx = config.resolve_context(&cwd).unwrap();
        assert_eq!(ctx.team, "PROD");
        assert_eq!(ctx.project.as_deref(), Some("Tokamak"));
        assert_eq!(ctx.label.as_deref(), Some("tokamak"));
    }

    // ERR-20: auto-detect team desde cwd acme
    #[test]
    fn test_resolve_context_acme() {
        let config = test_config();
        let home = dirs::home_dir().unwrap();
        let cwd = home.join("code/acme");
        let ctx = config.resolve_context(&cwd).unwrap();
        assert_eq!(ctx.team, "PROD");
        assert_eq!(ctx.label.as_deref(), Some("acme"));
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
        let cwd = home.join("code/tokamak");
        let (team, project, label) = config.resolve_team(Some("CONT"), &cwd).unwrap();
        assert_eq!(team, "CONT");
        assert!(project.is_none()); // override no trae project/label
        assert!(label.is_none());
    }

    // ERR-34: team retirado TOK
    #[test]
    fn test_retired_team_tok() {
        let config = test_config();
        let cwd = Path::new("/tmp");
        let result = config.resolve_team(Some("TOK"), &cwd);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("retired"));
    }

    // ERR-35: team retirado QIN
    #[test]
    fn test_retired_team_qin() {
        let config = test_config();
        let cwd = Path::new("/tmp");
        let result = config.resolve_team(Some("QIN"), &cwd);
        assert!(result.is_err());
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
        let cwd = home.join("code/tokamak/src/deep/nested");
        let ctx = config.resolve_context(&cwd).unwrap();
        assert_eq!(ctx.team, "PROD");
        assert_eq!(ctx.label.as_deref(), Some("tokamak"));
    }
}
