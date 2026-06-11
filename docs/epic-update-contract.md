# Epic update contract

This document specifies the missing `lql` surface that forced an agent to fall
back to `lql raw` when updating an epic's long-form plan.

## Problem

`lql epic view` can read the long markdown body of an epic, and `lql epic
create` correctly writes that body to Linear's `content` field. However, after
an epic exists, the public CLI only supports:

- `lql epic create`
- `lql epic list`
- `lql epic view`
- `lql epic add`

There is no first-class way to update the epic body or leave an epic-level
comment. In practice, agents have to use raw GraphQL:

```bash
lql raw 'mutation($id: String!, $content: String!) {
  projectUpdate(id: $id, input: { content: $content }) {
    success
  }
}' --var id=... --var content=...
```

That is exactly the kind of escape hatch `lql` is meant to make unnecessary:
the agent must discover internal IDs, know that the long body lives on the
backing project, and hand-write GraphQL.

## Required commands

### `lql epic update`

Update the Linear initiative and its single backing project through the epic
abstraction.

```bash
lql epic update <epic_id> [OPTIONS]
```

Supported options:

- `--title <TITLE>`: update the initiative name and backing project name.
  The backing project name must keep using the existing 80-character truncation
  rule.
- `-d, --description <BODY>`: replace the long markdown body.
- `--description-file <FILE>`: replace the long markdown body from a file.
- `--summary <TEXT>`: update the short Linear initiative description, not the
  long markdown body.
- `--target-date <YYYY-MM-DD>`: update the initiative target date if supported
  by the current schema.
- `--json`: optional compatibility with the existing script-oriented style.
  It is not required for the agent workflow.

Rules:

- At least one update option is required.
- `--description` and `--description-file` are mutually exclusive.
- The long markdown body must be written to `content`, not `description`.
- For `lql`-managed epics with one backing project, update both the initiative
  content and the backing project content so `lql epic view` and Linear's
  project page remain consistent.
- For epics with zero backing projects, create one with the current epic title
  and target teams before writing content, matching `lql epic add` behavior.
- For epics with more than one backing project, fail loud unless
  `--project <project-id-or-slug>` is supplied in a later version. MVP can
  reject this case.

Human output:

```text
✓ cb19ff35fa52 updated
  title, content
```

Agent-oriented compact output should stay line-oriented and TOON-compatible
with the rest of `lql`; do not introduce a new JSON-only contract for Claude:

```text
cb19ff35fa52{updated}:
  fields: title,content
  project: 28fe0617-0bee-4fcd-aad1-b1626197e22a
```

### `lql epic comment`

Add a comment to the epic's backing project, without requiring agents to know
the backing project ID.

```bash
lql epic comment <epic_id> [BODY]
lql epic comment <epic_id> --file comment.md
```

Rules:

- Positional body, `--body`, stdin, and `--file` should follow the same
  conventions as `lql comment`.
- If the epic has zero backing projects, fail with a hint to create/add a
  backing project first. Do not silently create a project just to hold a
  comment.
- If the epic has more than one backing project, fail loud in MVP.

Human output:

```text
✓ Comment added to cb19ff35fa52
```

### `lql epic view` compact contract

`lql epic view` already prints a useful text view with the long body and issue
list. The missing piece is not JSON; it is a stable compact contract that
exposes the backing project enough for follow-up commands and avoids raw
GraphQL.

Required default output properties:

- Epic slugId, status, title, URL, teams.
- Long markdown body from `content`.
- Backing project ID, name, slugId, URL, teams.
- Issue list in the existing compact/TOON issue format.

Example:

```text
cb19ff35fa52 [Active] PROD — Bastidor v1.0
  Projects: 1 | Issues: 12 | Teams: PROD
  https://linear.app/...
  ─────
  # Long markdown body
  ─────
  Projects: 28fe0617-0bee-4fcd-aad1-b1626197e22a "Bastidor v1.0" [PROD]
[12]{id,state,labels,title,priority,age,due,project}:
  "PROD-1130",Backlog,"acme,proxy,seo","Cutover blocker: live profiles 404 behind the new proxy",1,today,"","Proxy v1.0"
── 12 issues (12 backlog)
```

`--json` may continue to exist for scripts, but this feature must not depend on
Claude parsing JSON.

## Optional generic project commands

These are useful, but not required if `epic update` and `epic comment` cover the
agent workflow:

```bash
lql project view <project_id_or_name>
lql project update <project_id_or_name> --content-file plan.md
lql project comment <project_id_or_name> --file note.md
```

If implemented, project commands should use project IDs/slugs/names directly.
Agents should not need `lql raw` for `projectUpdate`.

## GraphQL operations

Claude should verify the current schema from `docs/linear-schema.md` before
coding. Expected operations:

- `initiativeUpdate(id: $id, input: InitiativeUpdateInput!)`
- `projectUpdate(id: $id, input: ProjectUpdateInput!)`
- comment creation against the backing project if Linear exposes project
  comments in the current schema. If Linear only comments on documents/issues,
  document the limitation and keep `epic comment` out of MVP.

Do not guess field names. Regenerate or inspect the schema before implementing.

## Acceptance tests

Add unit tests that do not call the real API:

- Clap accepts `lql epic update cb19ff35fa52 --description-file plan.md`.
- Clap rejects `--description` plus `--description-file`.
- `epic update` errors when no update flags are supplied.
- Long markdown body is written to `content`, never to `description`.
- Title update applies initiative name and truncated backing project name.
- Epic with zero projects creates a backing project before content update.
- Epic with more than one project fails with an actionable error.
- `epic view` default output includes the backing project ID/name/URL and keeps
  issues in the existing compact TOON format.

Add one integration test only if the Linear test workspace can safely create
and delete a temporary epic/project.

## Non-goals

- Full project management UX.
- Editing multiple backing projects in one command.
- Replacing `lql raw` entirely. `raw` remains an escape hatch, but it should not
  be necessary for normal epic planning workflows.
