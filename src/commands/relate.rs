use crate::cli::RelateOpts;
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
        "related" => Ok(NormalizedRelation {
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
    let client = Client::new(&config.auth.api_key_ref)?;

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
}
