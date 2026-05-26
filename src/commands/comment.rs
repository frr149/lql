use crate::cli::CommentOpts;
use crate::client::{Client, GraphQLClient};
use crate::commands::update::find_issue_by_identifier;
use crate::config::Config;
use std::io::{IsTerminal, Read};

pub fn run(config: &Config, opts: &CommentOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;

    // Resolver issue UUID
    let issue = find_issue_by_identifier(&client, &opts.issue_id)?;
    let issue_uuid = issue
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Could not get issue UUID")?;

    let is_terminal = std::io::stdin().is_terminal();
    let body = resolve_body(opts, &mut std::io::stdin(), is_terminal)?;

    let variables = serde_json::json!({
        "input": {
            "issueId": issue_uuid,
            "body": body,
        }
    });

    let data = client.query(crate::queries::COMMENT_MUTATION, variables)?;

    let success = data
        .get("commentCreate")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if success {
        println!("✓ Comment added to {}", opts.issue_id);
    } else {
        return Err(format!("Failed to add comment to {}", opts.issue_id));
    }

    Ok(())
}

/// Sources for a comment body, shared across `lql comment`, `lql epic comment`
/// and `lql project comment`.
pub struct CommentSource<'a> {
    pub body: Option<&'a str>,
    pub body_flag: Option<&'a str>,
    pub file: Option<&'a str>,
    /// Hint shown when no body source is provided on a TTY (so each command can
    /// say `lql comment …`, `lql epic comment …`, etc.).
    pub usage_hint: &'a str,
}

/// Resuelve el body del comentario: inline > --body flag > fichero > reader (stdin)
/// Extraído para ser testeable sin stdin real
pub fn resolve_body(
    opts: &CommentOpts,
    reader: &mut dyn Read,
    is_terminal: bool,
) -> Result<String, String> {
    resolve_body_from_source(
        &CommentSource {
            body: opts.body.as_deref(),
            body_flag: opts.body_flag.as_deref(),
            file: opts.file.as_deref(),
            usage_hint: "lql comment ID \"text\" or --file or stdin",
        },
        reader,
        is_terminal,
    )
}

/// Same precedence as `resolve_body`, but parametric in the source — used by
/// commands that don't own a `CommentOpts` (e.g. `epic comment`, `project
/// comment`).
pub fn resolve_body_from_source(
    source: &CommentSource<'_>,
    reader: &mut dyn Read,
    is_terminal: bool,
) -> Result<String, String> {
    let body = if let Some(text) = source.body {
        text.to_string()
    } else if let Some(text) = source.body_flag {
        text.to_string()
    } else if let Some(path) = source.file {
        std::fs::read_to_string(path)
            .map_err(|e| format!("Could not read file {path}: {e}"))?
    } else if !is_terminal {
        let mut buf = String::new();
        reader
            .read_to_string(&mut buf)
            .map_err(|e| format!("Could not read from stdin: {e}"))?;
        buf
    } else {
        return Err(format!(
            "No comment body provided. Use: {}",
            source.usage_hint
        ));
    };

    if body.trim().is_empty() {
        return Err("Comment body is empty".to_string());
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn opts_inline(body: &str) -> CommentOpts {
        CommentOpts {
            issue_id: "PROD-1".to_string(),
            body: Some(body.to_string()),
            body_flag: None,
            file: None,
        }
    }

    fn opts_no_body() -> CommentOpts {
        CommentOpts {
            issue_id: "PROD-1".to_string(),
            body: None,
            body_flag: None,
            file: None,
        }
    }

    // ERR-65: body inline funciona
    #[test]
    fn test_resolve_body_inline() {
        let opts = opts_inline("Investigado, el problema es X");
        let body = resolve_body(&opts, &mut Cursor::new(vec![]), true).unwrap();
        assert_eq!(body, "Investigado, el problema es X");
    }

    // ERR-67: body desde stdin (simulado con Cursor)
    #[test]
    fn test_resolve_body_stdin() {
        let opts = opts_no_body();
        let stdin_data = b"Progreso parcial";
        let body = resolve_body(&opts, &mut Cursor::new(stdin_data.to_vec()), false).unwrap();
        assert_eq!(body, "Progreso parcial");
    }

    // Sin body y terminal = error
    #[test]
    fn test_resolve_body_no_body_terminal() {
        let opts = opts_no_body();
        let err = resolve_body(&opts, &mut Cursor::new(vec![]), true).unwrap_err();
        assert!(err.contains("No comment body provided"), "{err}");
    }

    // Body vacío = error
    #[test]
    fn test_resolve_body_empty() {
        let opts = opts_inline("   ");
        let err = resolve_body(&opts, &mut Cursor::new(vec![]), true).unwrap_err();
        assert!(err.contains("Comment body is empty"), "{err}");
    }

    // Stdin vacío = error
    #[test]
    fn test_resolve_body_stdin_empty() {
        let opts = opts_no_body();
        let err = resolve_body(&opts, &mut Cursor::new(vec![]), false).unwrap_err();
        assert!(err.contains("Comment body is empty"), "{err}");
    }
}
