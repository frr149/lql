//! P13 meta-test: the test that watches the tests (TOOL-128).
//!
//! A critical test silenced with `#[ignore]` is invisible — it passes
//! `cargo test` by NOT running. This guard scans the repo's own test sources
//! and fails if any `#[ignore]` appears without a justified allowlist entry.
//! It is the "the lint is the project's memory" pattern applied to the test
//! corpus itself: switching a guard off becomes as loud as breaking the build.

use std::fs;
use std::path::{Path, PathBuf};

/// Every `#[ignore]`d test MUST appear here with a reason. A live smoke test may
/// stay ignored ONLY because a CI-runnable equivalent exists — name it.
const IGNORE_ALLOWLIST: &[(&str, &str)] = &[
    // The 14 integration tests hit the real Linear API (need LINEAR_API_KEY +
    // network), so they are live smoke tests, not merge-loop guards.
    (
        "integration_view_nonexistent_issue",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_search_no_results",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_search_finds_issues",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_concurrent_list",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_filter_flag_rejected",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_filter_flag_rejected_in_json_mode_uses_machine_error_prefix",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_doctor",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_labels",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_list_json",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_list_no_label",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_no_label_with_label_error",
        "live Linear API smoke; needs key+network",
    ),
    (
        "integration_context",
        "live Linear API smoke; needs key+network",
    ),
    // The epic guards stay live, BUT the PR #12 regression (GraphQL complexity +
    // UUID-filter) is ALSO guarded network-free in CI by queries::budget_tests
    // and epic::build_epic_ref_filter tests.
    (
        "integration_epic_list_succeeds",
        "live smoke; regression guarded in CI by queries::budget_tests + epic::build_epic_ref_filter tests",
    ),
    (
        "integration_epic_view_succeeds",
        "live smoke; regression guarded in CI by queries::budget_tests + epic::build_epic_ref_filter tests",
    ),
];

/// Returns the names of `#[ignore]`d tests found in `source` that are not in
/// `allow`. Pass an empty `allow` to get every ignored test.
fn unallowlisted_ignores(source: &str, allow: &[&str]) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut offenders = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        // Matches both `#[ignore]` and `#[ignore = "reason"]`. A string literal
        // containing the text is never line-leading, so it won't false-match.
        if line.trim_start().starts_with("#[ignore") {
            let upper = (i + 6).min(lines.len());
            if let Some(name) = lines[i + 1..upper].iter().find_map(|l| fn_name(l))
                && !allow.contains(&name.as_str())
            {
                offenders.push(name);
            }
        }
    }
    offenders
}

/// Extracts the identifier from a `fn NAME(...)` / `async fn NAME(...)` line.
fn fn_name(line: &str) -> Option<String> {
    let t = line.trim_start();
    let rest = t
        .strip_prefix("fn ")
        .or_else(|| t.strip_prefix("async fn "))?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// All `.rs` files under `src/` and `tests/`, excluding this meta-test file
/// (its source legitimately contains the literal `#[ignore` inside strings).
fn scanned_sources() -> Vec<PathBuf> {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut files = Vec::new();
    rust_sources(&Path::new(root).join("src"), &mut files);
    rust_sources(&Path::new(root).join("tests"), &mut files);
    let myself = Path::new(file!()).file_name();
    files.retain(|f| f.file_name() != myself);
    files
}

#[test]
fn no_unallowlisted_ignored_tests() {
    let allow: Vec<&str> = IGNORE_ALLOWLIST.iter().map(|(n, _)| *n).collect();
    let mut offenders = Vec::new();
    for f in scanned_sources() {
        let src = fs::read_to_string(&f).unwrap();
        for name in unallowlisted_ignores(&src, &allow) {
            offenders.push(format!("{}::{name}", f.display()));
        }
    }
    assert!(
        offenders.is_empty(),
        "P13 (TOOL-128): these tests are #[ignore]d but not in IGNORE_ALLOWLIST — a \
         critical guard may have been silently switched off. Either make the test run \
         in CI, or add a justified allowlist entry in tests/meta_tests.rs: {offenders:?}"
    );
}

/// Staleness: every allowlist entry must still match a real `#[ignore]`d test,
/// so the allowlist can't rot into names that no longer exist.
#[test]
fn allowlist_has_no_stale_entries() {
    let mut all_ignored: Vec<String> = Vec::new();
    for f in scanned_sources() {
        let src = fs::read_to_string(&f).unwrap();
        all_ignored.extend(unallowlisted_ignores(&src, &[]));
    }
    let stale: Vec<&str> = IGNORE_ALLOWLIST
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !all_ignored.iter().any(|x| x == n))
        .collect();
    assert!(
        stale.is_empty(),
        "stale IGNORE_ALLOWLIST entries (no matching #[ignore]d test remains): {stale:?}"
    );
}

/// Anti-guard / positive control (P06): the scanner MUST catch an un-allowlisted
/// ignore. If this ever passes vacuously, the meta-test is dead.
#[test]
fn meta_guard_detects_unallowlisted_ignore() {
    let synthetic = "#[test]\n#[ignore]\nfn sneaky_disabled_guard() { assert!(true); }\n";
    assert_eq!(
        unallowlisted_ignores(synthetic, &[]),
        vec!["sneaky_disabled_guard".to_string()]
    );
    // And it must respect the allowlist.
    assert!(unallowlisted_ignores(synthetic, &["sneaky_disabled_guard"]).is_empty());
}
