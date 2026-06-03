use crate::cli::{RelateOpts, UnlinkOpts};
use crate::client::{Client, GraphQLClient};
use crate::commands::update::find_issue_by_identifier;
use crate::config::Config;

/// Resultado de normalizar un tipo de relación
#[derive(Debug)]
pub struct NormalizedRelation {
    /// Tipo de relación para la API (blocks o related)
    pub api_type: &'static str,
    /// Si hay que invertir from/to (blocked-by → blocks invertido)
    pub invert: bool,
    /// Nombre para mostrar al usuario
    pub display: &'static str,
}

/// Normaliza el tipo de relación. Devuelve error si es inválido.
pub fn normalize_relation_type(input: &str) -> Result<NormalizedRelation, String> {
    match input.to_lowercase().as_str() {
        "blocks" => Ok(NormalizedRelation {
            api_type: "blocks",
            invert: false,
            display: "blocks",
        }),
        "blocked-by" | "blockedby" => Ok(NormalizedRelation {
            api_type: "blocks",
            invert: true,
            display: "blocked-by",
        }),
        "related" | "relates" | "relates-to" | "relatesto" => Ok(NormalizedRelation {
            api_type: "related",
            invert: false,
            display: "related",
        }),
        other => Err(format!(
            "Unknown relation type \"{other}\". Available: blocks, blocked-by, related"
        )),
    }
}

pub fn run(config: &Config, opts: &RelateOpts) -> Result<(), String> {
    // Si el tipo es "unlink", delegar a run_unlink
    if opts.relation_type.eq_ignore_ascii_case("unlink") {
        let unlink_opts = UnlinkOpts {
            from: opts.from.clone(),
            to: opts.to.clone(),
        };
        return run_unlink(config, &unlink_opts);
    }

    let client = Client::new(&config.auth)?;

    // Normalizar tipo de relación
    let norm = normalize_relation_type(&opts.relation_type)?;
    let (from_id, to_id) = if norm.invert {
        (&opts.to, &opts.from)
    } else {
        (&opts.from, &opts.to)
    };

    // Resolver UUIDs
    let from_issue = find_issue_by_identifier(&client, from_id)?;
    let to_issue = find_issue_by_identifier(&client, to_id)?;

    let from_uuid = from_issue
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Could not get source issue UUID")?;
    let to_uuid = to_issue
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Could not get target issue UUID")?;

    let variables = serde_json::json!({
        "input": {
            "issueId": from_uuid,
            "relatedIssueId": to_uuid,
            "type": norm.api_type,
        }
    });

    let data = client.query(crate::queries::RELATION_MUTATION, variables)?;

    let success = data
        .get("issueRelationCreate")
        .and_then(|r| r.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if success {
        println!("✓ {} {} {}", opts.from, norm.display, opts.to);
    } else {
        return Err(format!(
            "Failed to create relation {} {} {}",
            opts.from, norm.display, opts.to
        ));
    }

    Ok(())
}

/// Busca el ID de una relación entre dos issues (en cualquier dirección).
fn find_relation_id(
    client: &Client,
    from_identifier: &str,
    to_identifier: &str,
) -> Result<String, String> {
    let (team, number) = from_identifier
        .split_once('-')
        .ok_or_else(|| format!("Invalid identifier: {from_identifier}"))?;
    let number: i64 = number
        .parse()
        .map_err(|_| format!("Invalid issue number in {from_identifier}"))?;

    let variables = serde_json::json!({
        "filter": {
            "team": { "key": { "eq": team } },
            "number": { "eq": number }
        }
    });

    let data = client.query(crate::queries::ISSUE_RELATIONS_QUERY, variables)?;

    let issue = data
        .get("issues")
        .and_then(|i| i.get("nodes"))
        .and_then(|n| n.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| format!("Issue {from_identifier} not found"))?;

    // Buscar en relaciones directas (from → to)
    if let Some(relations) = issue
        .get("relations")
        .and_then(|r| r.get("nodes"))
        .and_then(|n| n.as_array())
    {
        for rel in relations {
            let target = rel
                .get("relatedIssue")
                .and_then(|ri| ri.get("identifier"))
                .and_then(|id| id.as_str())
                .unwrap_or("");
            if target.eq_ignore_ascii_case(to_identifier)
                && let Some(id) = rel.get("id").and_then(|id| id.as_str())
            {
                return Ok(id.to_string());
            }
        }
    }

    // Buscar en relaciones inversas (to → from, almacenada al revés)
    if let Some(inverse) = issue
        .get("inverseRelations")
        .and_then(|r| r.get("nodes"))
        .and_then(|n| n.as_array())
    {
        for rel in inverse {
            let source = rel
                .get("issue")
                .and_then(|ri| ri.get("identifier"))
                .and_then(|id| id.as_str())
                .unwrap_or("");
            if source.eq_ignore_ascii_case(to_identifier)
                && let Some(id) = rel.get("id").and_then(|id| id.as_str())
            {
                return Ok(id.to_string());
            }
        }
    }

    Err(format!(
        "No relation found between {from_identifier} and {to_identifier}"
    ))
}

pub fn run_unlink(config: &Config, opts: &UnlinkOpts) -> Result<(), String> {
    let client = Client::new(&config.auth)?;

    // Buscar la relación en ambas direcciones
    let relation_id = match find_relation_id(&client, &opts.from, &opts.to) {
        Ok(id) => id,
        Err(_) => {
            // Intentar en la dirección opuesta
            find_relation_id(&client, &opts.to, &opts.from)?
        }
    };

    let variables = serde_json::json!({ "id": relation_id });
    let data = client.query(crate::queries::RELATION_DELETE_MUTATION, variables)?;

    let success = data
        .get("issueRelationDelete")
        .and_then(|r| r.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if success {
        println!("✓ Unlinked {} ↔ {}", opts.from, opts.to);
    } else {
        return Err(format!(
            "Failed to remove relation between {} and {}",
            opts.from, opts.to
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ERR-68: blocks normaliza correctamente
    #[test]
    fn test_normalize_blocks() {
        let r = normalize_relation_type("blocks").unwrap();
        assert_eq!(r.api_type, "blocks");
        assert!(!r.invert);
        assert_eq!(r.display, "blocks");
    }

    // ERR-69: blocked-by invierte y normaliza
    #[test]
    fn test_normalize_blocked_by() {
        let r = normalize_relation_type("blocked-by").unwrap();
        assert_eq!(r.api_type, "blocks");
        assert!(r.invert);
        assert_eq!(r.display, "blocked-by");
    }

    // ERR-69b: blockedby (sin guión) también funciona
    #[test]
    fn test_normalize_blockedby_no_hyphen() {
        let r = normalize_relation_type("blockedby").unwrap();
        assert!(r.invert);
    }

    // ERR-70: related
    #[test]
    fn test_normalize_related() {
        let r = normalize_relation_type("related").unwrap();
        assert_eq!(r.api_type, "related");
        assert!(!r.invert);
    }

    // ERR-71: tipo inválido
    #[test]
    fn test_normalize_invalid_type() {
        let err = normalize_relation_type("depends-on").unwrap_err();
        assert!(err.contains("Unknown relation type"), "{err}");
        assert!(err.contains("blocks, blocked-by, related"), "{err}");
    }

    // Case insensitive
    #[test]
    fn test_normalize_case_insensitive() {
        assert!(normalize_relation_type("Blocks").is_ok());
        assert!(normalize_relation_type("RELATED").is_ok());
        assert!(normalize_relation_type("Blocked-By").is_ok());
    }

    // ===================================================================
    // Agentic experience tests — fixtures from real Claude Code sessions.
    // ===================================================================

    // --- AX-09: `relates` → should normalize to `related` ---
    // Real: 'error: Unknown relation type "relates"'
    #[test]
    fn test_normalize_relates_to_related() {
        let r = normalize_relation_type("relates").unwrap();
        assert_eq!(r.api_type, "related");
        assert!(!r.invert);
    }

    // --- AX-10: `lql relate PROD-834 PROD-833 blocked-by` ---
    // Reordering happens in middleware::normalize_args (pre-clap).
    // This test verifies the end-to-end result after middleware.
    #[test]
    fn test_relate_reorder_from_to_type() {
        use crate::cli::Cli;
        use crate::middleware;
        use clap::Parser;

        let raw = vec![
            "lql".to_string(),
            "relate".to_string(),
            "PROD-834".to_string(),
            "PROD-833".to_string(),
            "blocked-by".to_string(),
        ];
        let fixed = middleware::normalize_args(&raw).expect("should reorder");
        let cli = Cli::try_parse_from(&fixed).unwrap();
        if let crate::cli::Command::Relate(opts) = cli.command {
            let norm = normalize_relation_type(&opts.relation_type).unwrap();
            assert_eq!(norm.api_type, "blocks");
            assert!(norm.invert);
            assert_eq!(opts.from, "PROD-834");
            assert_eq!(opts.to, "PROD-833");
        } else {
            panic!("Expected Relate command");
        }
    }

    // --- AX-12: `relates-to` → should normalize to `related` ---
    // Real: 'error: Unknown relation type "relates-to"'
    #[test]
    fn test_normalize_relates_to_hyphenated() {
        let r = normalize_relation_type("relates-to").unwrap();
        assert_eq!(r.api_type, "related");
        assert!(!r.invert);
    }
}
