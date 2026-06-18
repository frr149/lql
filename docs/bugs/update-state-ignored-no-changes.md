# BUG: `lql update --state` silently ignored → "No changes specified"

- **Status:** open (fix deferred — this report + a reproduction test only)
- **Reported:** 2026-06-18
- **Component:** `update` subcommand
- **Severity:** high — `update --state` is the primary state-transition command and it
  fails for any workflow state whose name is not one of the hardcoded aliases.

## Reproduction

```console
$ lql update PROD-1244 --state "In Review"
error: No changes specified. Use --state, --priority, --label, --title, --project, --team, or --due.

$ lql update PROD-1244 --state="In Review"
error: No changes specified. Use --state, --priority, --label, --title, --project, --team, or --due.
```

`lql update --help` confirms the flag exists and the syntax is correct:

```
--state <STATE>   Change state
Usage: lql update [OPTIONS] <ISSUE_ID>
```

## Observed behaviour

Passing **only** `--state` triggers the `has_changes == false` guard, as if no change had
been requested at all. The state is never written and the command exits with an error.

## Expected behaviour

`lql update PROD-1244 --state "In Review"` should resolve the workflow state named
"In Review" on the issue's team and set `stateId` in the `issueUpdate` mutation. If the
state genuinely cannot be resolved, the command must fail with a **specific** error naming
the unknown state and listing the available states — **never** the misleading generic
"No changes specified", which implies the user forgot to pass any flag.

## Root cause

Two compounding defects:

### 1. Silent drop — `has_changes` is set inside the resolution branch

`src/commands/update.rs:42-54`

```rust
if let Some(ref state_str) = opts.state {
    let state_type = cli::normalize_state(state_str, &config.state_aliases);
    let effective_team = /* ... */;
    if let Some(state) = meta.find_state(effective_team, &state_type) {
        input["stateId"] = serde_json::json!(state.id);
        new_state_name = state.name.clone();
        has_changes = true;          // <-- only set INSIDE the Some(state) arm
    }
    // <-- no `else`: when find_state returns None, the flag is dropped silently
}
```

Every other flag (`--team`, `--priority`, `--project`, `--label`, `--title`,
`--description`, `--due`) sets `has_changes = true` unconditionally, or propagates a real
error via `?`. `--state` is the **only** flag that can be silently swallowed: when
`find_state` returns `None`, `has_changes` stays `false` and control falls through to the
guard at `src/commands/update.rs:128-133`, producing the misleading message.

### 2. `find_state` matches by category, `normalize_state` only knows 5 aliases

`src/client.rs:224-226`

```rust
pub fn find_state<'a>(&self, team: &'a TeamInfo, state_type: &str) -> Option<&'a StateInfo> {
    team.states.iter().find(move |s| s.state_type == state_type)
}
```

`find_state` matches on `state_type`, i.e. Linear's workflow **category**
(`backlog` / `unstarted` / `started` / `completed` / `canceled`), **not** the state's
human-facing **display name**.

`normalize_state` (`src/cli.rs:677-700`) maps a fixed alias set
(`Todo→unstarted`, `In Progress→started`, `Done→completed`, `Canceled/Cancelled→canceled`)
plus the five raw API category values; anything else is returned lowercased verbatim
(`src/cli.rs:699`).

So `--state "In Review"`:

1. `normalize_state("In Review", aliases)` → no alias hit, not a category value →
   returns `"in review"` (raw, lowercased).
2. `find_state(team, "in review")` → no state has `state_type == "in review"` (categories
   are never "in review") → returns `None`.
3. `has_changes` stays `false` → "No changes specified."

"In Review" is a common **custom** workflow state in the `started` category. There is no
way, with the current code, to target a workflow state by its display name — only the five
hardcoded aliases (and raw category values) work. Any team using custom state names
(In Review, In QA, Blocked, Triage, …) cannot be transitioned to them via `--state`.

## Scope

- **`--state` only** is hit by the _silent-drop_ defect (#1). All other update flags set
  `has_changes` unconditionally or error out, so they are not silently swallowed.
- Defect #2 (resolve-by-category, not by name) affects **any** state whose display name is
  not one of the five hardcoded aliases — independent of the silent-drop bug.

## Suggested fix (deferred — not applied here)

Resolve workflow states by display name (case-insensitive) **in addition to** category, and
make a failed resolution a hard, specific error instead of a silent no-op. Concretely:

- In `update.rs`, move `has_changes = true` out of the `if let Some(state)` arm, or add an
  `else` that returns an error naming the unresolved state and listing `team.states`.
- Add a `find_state_by_name` on `LinearMeta` (match `s.name.eq_ignore_ascii_case(name)`),
  and try it before/after the category match in `normalize_state`/`find_state`.

A reproduction test is included at `src/commands/update.rs`
(`test_state_by_display_name_is_dropped_bug`, `#[ignore]`d and allowlisted in
`tests/meta_tests.rs`). Remove the `#[ignore]` and its allowlist entry when fixing.
