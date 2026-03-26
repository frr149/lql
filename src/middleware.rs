/// Middleware pre-clap: intercepta flags erróneos comunes y sugiere la alternativa correcta.
/// Contrato de Sancho Panza: adapta lo no destructivo, rechaza lo destructivo, siempre avisa.

/// Comprueba args comunes que clap rechazaría con un mensaje genérico inútil.
/// Devuelve Err(mensaje útil) si detecta un flag conocido-erróneo.
pub fn check_common_mistakes(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Ok(());
    }

    let subcommand = args[1].as_str();

    for (i, arg) in args.iter().enumerate().skip(2) {
        match arg.as_str() {
            "--filter" => {
                return Err(
                    "--filter no existe. Para filtrar por estado: --state <estado>. Para buscar texto: lql search \"texto\""
                        .to_string(),
                );
            }
            "--query" => {
                let value = args.get(i + 1).map(|s| s.as_str()).unwrap_or("<texto>");
                return Err(format!(
                    "--query no existe en \"{subcommand}\". ¿Querías lql search \"{value}\"?"
                ));
            }
            "--no-limit" => {
                return Err(
                    "--no-limit no existe. Usa --limit 0 o --all para todos los resultados."
                        .to_string(),
                );
            }
            "--relates-to" => {
                let value = args.get(i + 1).map(|s| s.as_str()).unwrap_or("<ISSUE>");
                if subcommand == "update" {
                    let issue_id = args.get(2).map(|s| s.as_str()).unwrap_or("<FROM>");
                    return Err(format!(
                        "--relates-to no existe en update. Usa: lql relate {issue_id} related {value}"
                    ));
                }
                return Err(format!(
                    "--relates-to no existe. Usa: lql relate <FROM> related {value}"
                ));
            }
            "--comment" => {
                if subcommand == "update" {
                    let issue_id = args.get(2).map(|s| s.as_str()).unwrap_or("<ISSUE>");
                    let value = args.get(i + 1).map(|s| s.as_str()).unwrap_or("<texto>");
                    return Err(format!(
                        "--comment no existe en update. Usa: lql comment {issue_id} \"{value}\""
                    ));
                }
            }
            _ => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(slice: &[&str]) -> Vec<String> {
        slice.iter().map(|s| s.to_string()).collect()
    }

    // ERR-14: --filter → sugiere --state o search
    #[test]
    fn test_filter_rejected_with_guidance() {
        let err = check_common_mistakes(&args(&["lql", "list", "--filter", "backlog"])).unwrap_err();
        assert!(err.contains("--filter no existe"), "{err}");
        assert!(err.contains("--state"), "{err}");
        assert!(err.contains("lql search"), "{err}");
    }

    // ERR-15: --query → sugiere lql search con el valor
    #[test]
    fn test_query_rejected_with_guidance() {
        let err = check_common_mistakes(&args(&["lql", "list", "--query", "basedpyright"]))
            .unwrap_err();
        assert!(err.contains("--query no existe"), "{err}");
        assert!(err.contains("lql search \"basedpyright\""), "{err}");
    }

    // ERR-16: --no-limit → sugiere --limit 0 o --all
    #[test]
    fn test_no_limit_rejected_with_guidance() {
        let err = check_common_mistakes(&args(&["lql", "list", "--no-limit"])).unwrap_err();
        assert!(err.contains("--no-limit no existe"), "{err}");
        assert!(err.contains("--limit 0"), "{err}");
        assert!(err.contains("--all"), "{err}");
    }

    // ERR-17: --relates-to en update → sugiere lql relate
    #[test]
    fn test_relates_to_rejected_with_guidance() {
        let err = check_common_mistakes(&args(&[
            "lql",
            "update",
            "PROD-587",
            "--relates-to",
            "PROD-588",
        ]))
        .unwrap_err();
        assert!(err.contains("--relates-to no existe"), "{err}");
        assert!(err.contains("lql relate PROD-587 related PROD-588"), "{err}");
    }

    // ERR-18: --comment en update → sugiere lql comment
    #[test]
    fn test_comment_in_update_rejected_with_guidance() {
        let err = check_common_mistakes(&args(&[
            "lql",
            "update",
            "PROD-587",
            "--comment",
            "texto",
        ]))
        .unwrap_err();
        assert!(err.contains("--comment no existe en update"), "{err}");
        assert!(err.contains("lql comment PROD-587 \"texto\""), "{err}");
    }

    // Flags válidos no se interceptan
    #[test]
    fn test_valid_flags_pass_through() {
        assert!(check_common_mistakes(&args(&["lql", "list", "--state", "backlog"])).is_ok());
        assert!(check_common_mistakes(&args(&["lql", "list", "--all"])).is_ok());
        assert!(check_common_mistakes(&args(&["lql", "update", "PROD-1", "--state", "Done"])).is_ok());
    }

    // Sin subcomando no falla
    #[test]
    fn test_no_subcommand_ok() {
        assert!(check_common_mistakes(&args(&["lql"])).is_ok());
        assert!(check_common_mistakes(&args(&[])).is_ok());
    }

    // --comment fuera de update no se intercepta
    #[test]
    fn test_comment_outside_update_passes() {
        assert!(check_common_mistakes(&args(&["lql", "list", "--comment", "foo"])).is_ok());
    }

    // --relates-to fuera de update también se intercepta (nunca es válido)
    #[test]
    fn test_relates_to_outside_update_rejected() {
        let err = check_common_mistakes(&args(&["lql", "list", "--relates-to", "PROD-1"]))
            .unwrap_err();
        assert!(err.contains("--relates-to no existe"), "{err}");
    }
}
