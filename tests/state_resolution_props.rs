//! Property-based tests for `LinearMeta::find_state_for_mutation`.
//!
//! These pin the /types V8 fix for state resolution:
//! - display **name** always round-trips to its own state (identity preserved),
//! - category **cardinality** is honored (0 → not found, 1 → that state,
//!   ≥2 → ambiguous error — never a silent first-of-category pick),
//! - the verdict never depends on the **order** of `team.states` (no `.first()`).
//!
//! See `src/client.rs` and `docs/bugs/update-state-ignored-no-changes.md`.

use lql::client::{LinearMeta, StateInfo, TeamInfo};
use proptest::prelude::*;
use std::collections::HashMap;

const CATEGORIES: [&str; 5] = ["backlog", "unstarted", "started", "completed", "canceled"];

fn aliases() -> HashMap<String, String> {
    HashMap::from([
        ("Todo".into(), "unstarted".into()),
        ("In Progress".into(), "started".into()),
        ("Done".into(), "completed".into()),
        ("Canceled".into(), "canceled".into()),
        ("Cancelled".into(), "canceled".into()),
    ])
}

/// Build a team whose states have unique display names (`S0`, `S1`, …) — never a
/// category literal — and the given category per index.
fn team_from(cats: &[usize]) -> TeamInfo {
    let states = cats
        .iter()
        .enumerate()
        .map(|(i, &c)| StateInfo {
            id: format!("id{i}"),
            name: format!("S{i}"),
            state_type: CATEGORIES[c].to_string(),
        })
        .collect();
    TeamInfo {
        id: "team".into(),
        key: "PROD".into(),
        name: "Product".into(),
        states,
        projects: vec![],
    }
}

proptest! {
    /// R1 — name round-trip: every display name resolves to its own state.
    #[test]
    fn prop_name_roundtrip(cats in prop::collection::vec(0usize..5, 1..8)) {
        let team = team_from(&cats);
        let meta = LinearMeta { teams: vec![team.clone()], labels: vec![] };
        for s in &team.states {
            let got = meta
                .find_state_for_mutation(&team, &s.name, &aliases())
                .expect("a display name must always resolve");
            prop_assert_eq!(&got.id, &s.id);
        }
    }

    /// Cardinality + permutation invariance on the category-resolution path.
    #[test]
    fn prop_category_cardinality_and_permutation(
        cats in prop::collection::vec(0usize..5, 1..8)
    ) {
        let team = team_from(&cats);
        let mut reversed = team.states.clone();
        reversed.reverse();
        let team_rev = TeamInfo { states: reversed, ..team.clone() };
        let meta = LinearMeta { teams: vec![team.clone()], labels: vec![] };
        let meta_rev = LinearMeta { teams: vec![team_rev.clone()], labels: vec![] };

        for (ci, cat) in CATEGORIES.iter().enumerate() {
            let count = cats.iter().filter(|&&c| c == ci).count();
            let r1 = meta.find_state_for_mutation(&team, cat, &aliases());
            let r2 = meta_rev.find_state_for_mutation(&team_rev, cat, &aliases());
            match count {
                0 => {
                    prop_assert!(r1.is_err(), "empty category must be not-found");
                    prop_assert!(r2.is_err());
                }
                1 => {
                    let id1 = r1.expect("single-member category resolves").id.clone();
                    let id2 = r2.expect("single-member category resolves").id.clone();
                    prop_assert_eq!(id1, id2, "result must not depend on state order");
                }
                _ => {
                    prop_assert!(r1.is_err(), "ambiguous category must error, not pick");
                    prop_assert!(r2.is_err(), "ambiguous category must error, not pick");
                }
            }
        }
    }
}
