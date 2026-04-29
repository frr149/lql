fn looks_like_issue_id(s: &str) -> bool {
    let Some((team, num)) = s.split_once('-') else {
        return false;
    };
    !team.is_empty()
        && team.chars().all(|c| c.is_ascii_uppercase())
        && num.parse::<u32>().is_ok()
}

/// Reorder args for known patterns where agents swap positional arguments.
/// Returns a new Vec if reordering was needed, None otherwise.
pub fn normalize_args(args: &[String]) -> Option<Vec<String>> {
    if args.len() < 5 {
        return None;
    }
    // `relate ISSUE-ID ISSUE-ID TYPE` → `relate ISSUE-ID TYPE ISSUE-ID`
    if args[1] == "relate"
        && looks_like_issue_id(&args[2])
        && looks_like_issue_id(&args[3])
        && !looks_like_issue_id(&args[4])
    {
        let mut fixed = args.to_vec();
        fixed.swap(3, 4);
        eprintln!(
            "ℹ Reordered: relate {} {} {} → relate {} {} {}",
            args[2], args[3], args[4], fixed[2], fixed[3], fixed[4]
        );
        return Some(fixed);
    }
    None
}

/// Pre-clap middleware: intercept common flag mistakes and suggest the correct alternative.
/// Principle: adapt non-destructive input, reject destructive input, always inform.
pub fn check_common_mistakes(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Ok(());
    }

    let subcommand = args[1].as_str();

    for (i, arg) in args.iter().enumerate().skip(2) {
        match arg.as_str() {
            "--filter" => {
                return Err(
                    "--filter does not exist. To filter by state: --state <state>. To search: lql search \"text\""
                        .to_string(),
                );
            }
            "--query" => {
                let value = args.get(i + 1).map(|s| s.as_str()).unwrap_or("<text>");
                return Err(format!(
                    "--query does not exist in \"{subcommand}\". Did you mean: lql search \"{value}\"?"
                ));
            }
            "--no-limit" => {
                return Err(
                    "--no-limit does not exist. Use --limit 0 or --all for all results."
                        .to_string(),
                );
            }
            "--relates-to" => {
                let value = args.get(i + 1).map(|s| s.as_str()).unwrap_or("<ISSUE>");
                if subcommand == "update" {
                    let issue_id = args.get(2).map(|s| s.as_str()).unwrap_or("<FROM>");
                    return Err(format!(
                        "--relates-to does not exist in update. Use: lql relate {issue_id} related {value}"
                    ));
                }
                return Err(format!(
                    "--relates-to does not exist. Use: lql relate <FROM> related {value}"
                ));
            }
            "--comment" => {
                if subcommand == "update" {
                    let issue_id = args.get(2).map(|s| s.as_str()).unwrap_or("<ISSUE>");
                    let value = args.get(i + 1).map(|s| s.as_str()).unwrap_or("<text>");
                    return Err(format!(
                        "--comment does not exist in update. Use: lql comment {issue_id} \"{value}\""
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

    // ERR-14: --filter → suggests --state or search
    #[test]
    fn test_filter_rejected_with_guidance() {
        let err = check_common_mistakes(&args(&["lql", "list", "--filter", "backlog"])).unwrap_err();
        assert!(err.contains("--filter does not exist"), "{err}");
        assert!(err.contains("--state"), "{err}");
        assert!(err.contains("lql search"), "{err}");
    }

    // ERR-15: --query → suggests lql search with value
    #[test]
    fn test_query_rejected_with_guidance() {
        let err = check_common_mistakes(&args(&["lql", "list", "--query", "basedpyright"]))
            .unwrap_err();
        assert!(err.contains("--query does not exist"), "{err}");
        assert!(err.contains("lql search \"basedpyright\""), "{err}");
    }

    // ERR-16: --no-limit → suggests --limit 0 or --all
    #[test]
    fn test_no_limit_rejected_with_guidance() {
        let err = check_common_mistakes(&args(&["lql", "list", "--no-limit"])).unwrap_err();
        assert!(err.contains("--no-limit does not exist"), "{err}");
        assert!(err.contains("--limit 0"), "{err}");
        assert!(err.contains("--all"), "{err}");
    }

    // ERR-17: --relates-to in update → suggests lql relate
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
        assert!(err.contains("--relates-to does not exist"), "{err}");
        assert!(err.contains("lql relate PROD-587 related PROD-588"), "{err}");
    }

    // ERR-18: --comment in update → suggests lql comment
    #[test]
    fn test_comment_in_update_rejected_with_guidance() {
        let err = check_common_mistakes(&args(&[
            "lql",
            "update",
            "PROD-587",
            "--comment",
            "text",
        ]))
        .unwrap_err();
        assert!(err.contains("--comment does not exist in update"), "{err}");
        assert!(err.contains("lql comment PROD-587 \"text\""), "{err}");
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
        assert!(err.contains("--relates-to does not exist"), "{err}");
    }

    // ===================================================================
    // Agentic experience: arg reordering
    // ===================================================================

    #[test]
    fn test_looks_like_issue_id() {
        assert!(looks_like_issue_id("PROD-587"));
        assert!(looks_like_issue_id("TOOL-33"));
        assert!(looks_like_issue_id("KC-1"));
        assert!(!looks_like_issue_id("blocks"));
        assert!(!looks_like_issue_id("blocked-by"));
        assert!(!looks_like_issue_id("related"));
        assert!(!looks_like_issue_id("prod-587")); // lowercase team → not an ID
        assert!(!looks_like_issue_id("123"));
        assert!(!looks_like_issue_id(""));
    }

    // AX-10: `relate PROD-834 PROD-833 blocked-by` → reorder to `relate PROD-834 blocked-by PROD-833`
    #[test]
    fn test_normalize_args_relate_reorder() {
        let input = args(&["lql", "relate", "PROD-834", "PROD-833", "blocked-by"]);
        let result = normalize_args(&input).unwrap();
        assert_eq!(result[2], "PROD-834");
        assert_eq!(result[3], "blocked-by");
        assert_eq!(result[4], "PROD-833");
    }

    // Correct order should not be reordered
    #[test]
    fn test_normalize_args_correct_order_unchanged() {
        let input = args(&["lql", "relate", "PROD-834", "blocked-by", "PROD-833"]);
        assert!(normalize_args(&input).is_none());
    }

    // Non-relate commands not affected
    #[test]
    fn test_normalize_args_non_relate_unchanged() {
        let input = args(&["lql", "list", "--team", "PROD", "--limit", "5"]);
        assert!(normalize_args(&input).is_none());
    }
}
