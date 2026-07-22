# PRD — Semantic honesty: close raw-GraphQL CRUD gaps + config default team

- **Issue:** https://github.com/frr149/lql/issues/28
- **Branch:** `fix/semantic-honesty`
- **Status:** implemented (T01–T03 code + tests; T04 satisfied by existing tests)

## Context

lql is a Linear CLI whose primary consumer is an LLM (TOON output by default,
`--json` escape hatch — see `docs/PRD.md`, `README.md`). Its guiding invariant
for that agentic surface is **semantic honesty**, the name adopted in
`docs/reviews/codex-meta-opinion-2026-06-19.md` (explicitly *not* "agentic
experience" — the failure mode is observable incorrectness, not presentation):

> every explicit intent must produce an observable effect or a typed error;
> `exit 0` must describe the real effect, not merely that the API accepted a
> request; no error may blame the consumer for an omission that happened inside
> the tool.

lql is **already largely fail-loud**: `lib.rs::run` maps every `Err` to exit
code 1 + a stderr line, in machine mode when piped. This PRD does **not**
re-architect that. It closes the two concrete gaps that forced a real session
(2026-07-22, team KC) into hand-written `raw` GraphQL — which is dangerous:
passing markdown-with-backticks as an inline double-quoted shell string caused
command substitution that **corrupted a real Linear comment** — plus one small
ergonomic fallback, and locks the already-correct fail-loud behavior with a
regression property. Nothing more (YAGNI).

## Non-goals

- No rewrite of error handling / exit-code plumbing — it is already correct.
- No "infer team from a label unique to one team" (needs a metadata fetch and
  ambiguity handling; a config default is the boring fix).
- No fix for `cmd | jq` losing the exit code — that is shell pipeline semantics
  (`$pipestatus` / `pipefail`), not something lql can control.
- No new project fields beyond what `projectCreate` minimally needs.
- **No change to `lql search`.** Investigated: `search` does not call
  `resolve_team` and does not hard-fail on a missing team — with no `--team` it
  searches across all teams (a legitimate global search). The `--team KC` used in
  the seed session was unnecessary/defensive, not a bug. `search` is therefore
  **not** part of the team-detection fix.

---

## Tasks

### T01: Config `[defaults] team` fallback

**Objetivo**: When no `--team` is given and the cwd matches no `[context-map]`
entry, fall back to a configured default team before erroring.

**Depende de**: nothing.

**Entregable**: `src/config.rs` — add optional `team: Option<String>` to
`Defaults`; add a `TeamSource { Override, Context, Default }` enum; `resolve_team`
returns the source as a 4th tuple element and applies order `override >
context-map > default > error` (the default runs through the retired-team check).
Add a pure `team_fallback_warning(team) -> String`. `src/lib.rs` — add
`pub fn print_warning(msg, machine_mode)` mirroring `print_error` (`warning: …`
in machine mode, `⚠ …` otherwise), always to **stderr**. Callers
(`list.rs`, `create.rs`, `epic.rs`) emit the warning when source is `Default`.
`config.example.toml` — document `[defaults] team`.

**Criterios de aceptación**:

- With `[defaults] team = "KC"` and a cwd matching no context-map entry,
  `resolve_team(None, cwd)` returns team `"KC"` with source `TeamSource::Default`.
- `--team` override still wins (source `Override`); context-map match still wins
  (source `Context`).
- Retired-team check fires for an explicit `--team` **and** for a default that
  names a retired team (same error). [Sheldon #4]
- With **no** default and no match, the existing `Could not detect team …` error
  is returned unchanged (byte-for-byte).
- **When the default is used, a warning is emitted to stderr** (never stdout,
  which is TOON/machine output) naming the substituted team; `team_fallback_warning`
  mentions the team and that it is the configured default; `print_warning`
  formats `warning: …` in machine mode and `⚠ …` otherwise.

**Tests**:

- test_resolve_team_falls_back_to_default (source == Default)
- test_resolve_team_override_beats_default (source == Override)
- test_resolve_team_context_map_beats_default (source == Context)
- test_resolve_team_default_retired_is_rejected
- test_resolve_team_no_default_no_match_errors_unchanged
- test_team_fallback_warning_names_team
- test_print_warning_machine_and_human_format

### T02: `lql project create`

**Objetivo**: First-class project creation so users never hand-write
`projectCreate` GraphQL.

**Depende de**: nothing (may reuse T01 for team resolution).

**Entregable**: `src/cli.rs` — add `Create(ProjectCreateOpts)` to `ProjectAction`
(`--team`, `--description` / `--description-file`, positional name).
`src/commands/project.rs` — `run_create` that does the **I/O** (resolve team,
read description from file/stdin via `get_description_from_args`) and then calls
**REUSE, don't rebuild.** `epic.rs` already has all the machinery [verified,
Penny]: `PROJECT_CREATE_MUTATION` (queries.rs:381, `success` + `project`),
`build_project_input(title, body, team_ids)` (epic.rs:766, the input builder),
the 80-char cap validation (epic.rs:744), and a `FakeClient` test harness with a
`success:false → Err` test (epic.rs:1020, 1113). So T02 is mostly **wiring an
existing helper to a new subcommand** — no new mutation, no new "pure planner"
abstraction. `run_create` does the I/O (resolve team, read description via
`get_description_from_args`), calls `build_project_input`, sends
`PROJECT_CREATE_MUTATION`, and reads success from the response node.

**Criterios de aceptación**:

- `lql project create "Name" --team KC` sends `PROJECT_CREATE_MUTATION` with an
  input carrying `name` and `teamIds: ["<resolved team id>"]`.
- `--description` / `--description-file` are mutually exclusive; violation is a
  typed error (reuse `get_description_from_args`).
- Team is resolved via `resolve_team` (so T01's default — including its
  retired-team check — applies).
- Name length > Linear's 80-char project cap is a typed error before any request
  (reuse the existing epic validation, don't duplicate the constant).
- **Success output is read from the response node** (`project.name`/`id`), not
  from the local `name` argument; `--json` emits the raw node. [Sheldon #5]
- A server-side `success: false` **or a missing `project` node** is a
  non-zero-exit error, never `exit 0`. [Sheldon #5]

**Tests** (reuse the existing `FakeClient` pattern from epic.rs — no new harness):

- test_project_create_input_has_name_and_team
- test_project_create_description_flags_mutually_exclusive
- test_project_create_rejects_overlong_name
- test_project_create_reads_name_from_response (FakeClient returns a different
  name than requested → output shows the server's) [Sheldon #5]
- test_project_create_success_false_is_error (FakeClient: `success:false` → `Err`)
- test_project_create_cli_parse (arg ordering, from a real-session fixture)

### T03: `lql comment delete` + surface comment IDs

**Objetivo**: First-class comment deletion, and make comment IDs visible so the
delete is usable without dropping to `raw`/`--json`.

**Depende de**: nothing.

**Entregable**: `src/cli.rs` — split `Comment` into an action enum (or add a
`comment delete <id>` path) taking a comment id. `src/commands/comment.rs` —
`run_delete` calling a `COMMENT_DELETE_MUTATION`; success derived from
`commentDelete.success`. `src/format.rs` — `format_comments` prints each
comment's `id` (already fetched in `queries.rs`, currently dropped).

**Criterios de aceptación**:

- `lql comment delete <comment-id>` issues `commentDelete(id:)` with the id as
  the exact mutation variable, and reports success from the server's `success`
  field (not from the request having been sent). [Sheldon #5]
- `success: false` **or a missing `commentDelete`/`success` field** is a
  non-zero-exit error. [Sheldon #5]
- `lql comments <ISSUE>` output includes each comment's id in the default
  (non-`--json`) view. (The query always fetches `id`; no special-casing for a
  missing id — dropped per Penny, it designs UX for an impossible response.)
- Existing `lql comment <ISSUE> "text"` create path is unchanged.

**Tests** (reuse the `FakeClient` pattern):

- test_format_comments_includes_id
- test_comment_delete_sends_exact_id_variable (FakeClient asserts the id variable)
- test_comment_delete_success_reported_from_server
- test_comment_delete_missing_success_field_is_error
- test_comment_delete_cli_parse

### T04: Regression test — `--project` mismatch is a typed error (no silent no-op)

**Objetivo**: Pin the already-correct behavior that a `--project` name mismatch
errors (rather than silently no-opping the assignment), so a refactor can't
regress it. This is the sole surviving piece of the original BUG 2 lens.

**Depende de**: nothing (`find_project` already exists).

**Entregable**: none — **already covered** [verified during implementation]. The
absent-name → `Err` invariant is pinned by the pre-existing
`src/client.rs::test_project_not_found` (ERR-32, asserts `not found` + lists
`Available:`), exercised by both `create.rs` and `update.rs` `--project` via
`find_project`. The "no false success" half is now covered at the command
boundary by the T02/T03 response-truth tests (`project_from_create_response`,
`comment_delete_succeeded`). So T04 adds no new code: no proptest file, no
client-injection refactor, no stdout-capture. [Penny: `rewrite-smaller`; Penny
was correct that the case was already unit-tested.]

Rationale for the cut: the "errors never write to stdout" guarantee holds **by
construction** — `lib.rs::run` prints the success line only on the `Ok` path and
routes every `Err` to `print_error` (stderr). The response-truth cases
(`success:false` → `Err`) are already covered at the command boundary by the
T02/T03 `FakeClient` tests. A dedicated stdout-capture harness + DI refactor is
incidental complexity for a solo dev (Penny), and both reviewers reconcile here:
Sheldon's response-truth demand is satisfied by the T02/T03 fake-client tests,
which is where the real risk lives.

**Criterios de aceptación**:

- `find_project(team, name)` with a `name` absent from `team.projects` returns
  `Err` listing available names — satisfied by the existing `test_project_not_found`.

**Tests**:

- test_project_not_found (pre-existing, `src/client.rs`) — no new test added.

---

## Verification

- `cargo test` green: 277 lib + 16 fixture + 3 meta + 2 property tests pass; 14
  network-gated integration tests ignored (as before).
- `cargo clippy --all-targets -- -D warnings` clean.
- Manual smoke (from a non-linked cwd, with `[defaults] team = "KC"`):
  `lql project create "X" --description "y"`, `lql comments <ISSUE>` shows ids,
  `lql comment delete <id>`.

## Adversarial review outcome (verify mode)

`/sheldon` on the diff (Codex/gpt-5.6-sol) returned `incorrecto`:

- **BLOQUEANTE — fixed.** `project_from_create_response` treated a `success:true`
  response with an explicit `project: null` node as success (`.get("project")`
  yields `Some(Value::Null)` → `Ok`), faking a creation. Now `null` is filtered
  to `Err`; regression test `test_project_create_null_node_is_error` added.
- **IMPORTANTE — fixed.** Project-name validation ran *after* `LinearMeta::fetch`;
  moved to `check_project_name` called before any network request.
- **IMPORTANTE — fixed (FIX A).** The retired-team guard was a case-sensitive map
  lookup, so a lowercase default (`team = "tok"`) skipped the "retired" hint.
  Now the check is **case-insensitive on both the `--team` override and the
  `[defaults] team` paths** (Postel's law — `tok`/`TOK`/`Tok` all trip the hint),
  via `Config::retired_team_message`. Tests:
  `test_retired_team_override_case_insensitive`,
  `test_resolve_team_default_retired_case_insensitive`.
