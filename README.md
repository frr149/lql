# lql — Linear Query Language

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.93%2B-orange.svg)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/tests-305%20passing-brightgreen.svg)](#testing)
[![Binary Size](https://img.shields.io/badge/binary-4.7%20MB-informational.svg)](#install)
[![Built with Adversarial Programming](https://img.shields.io/badge/built%20with-adversarial%20programming-black.svg)](https://frr.dev/en/the-wrong-path-should-be-impossible-not-forbidden/)

A CLI for [Linear](https://linear.app) built for AI agents. Written in Rust.

```
$ lql list --team PROD --state Todo --priority urgent
[3]{id,state,labels,title,priority,age,due,project}:
  "PROD-587",Todo,tokamak,"Fix auth token refresh",1,14d,"Mar 28","Tokamak"
  "PROD-612",Todo,backend,"Migrate database schema",1,2d,"Apr 01",""
  "PROD-501",Todo,frontend,"Importar sesiones desde backup",1,30d,"overdue!",""
── 3 issues (3 todo)
```

## Why this exists

I work with a coding agent (Claude Code) that interacts with Linear ~800 times a month. An analysis of 165 sessions revealed **500+ errors and 370+ retries** caused by:

- Flags the agent kept inventing (`--status` instead of `--state`, `--priority urgent` instead of `--priority 1`)
- Operations the existing CLI couldn't do (search, filter by project, assign project on create)
- Verbose output wasting context tokens (~70 tokens/issue in JSON vs ~25 in lql)
- Mandatory flags the agent kept forgetting (`--sort`, `--no-pager`, `--no-interactive`)

Conservative estimate: **700K tokens/month wasted** just fighting with the interface.

The solution wasn't better documentation. It was a better tool.

## Design philosophy: the wrong path should be impossible, not forbidden

lql is built on a principle from [adversarial programming](https://frr.dev/en/the-wrong-path-should-be-impossible-not-forbidden/): don't tell an AI agent what not to do — make it so the wrong thing can't happen.

### Tolerance, not rejection

Instead of failing on reasonable input, lql normalizes it:

```
--status Done      →  --state completed    (silent alias)
--state Todo       →  --state unstarted    (with ℹ message)
--priority urgent  →  --priority 1         (with ℹ message)
--sort updated     →  --sort updatedAt     (with ℹ message)
```

Instead of cryptic errors for invalid flags, lql suggests the right command:

```
$ lql list --filter backlog
✗ --filter does not exist. To filter by state: --state <state>. To search: lql search "text"

$ lql update PROD-42 --comment "fix applied"
✗ --comment does not exist in update. Use: lql comment PROD-42 "fix applied"
```

### Sensible defaults, not mandatory flags

```bash
lql list                    # works. sorts by priority, active states, auto-detects team from cwd
lql create "Fix auth bug"   # works. auto-detects team, project, label from cwd
```

No `--sort`. No `--no-pager`. No `--no-interactive`. No `--all-assignees`. They're all defaults.

### Token-efficient output

The primary consumer is an LLM reading the output. lql uses [TOON](https://toonformat.dev/) (Token-Oriented Object Notation) — a compact format that encodes the schema once in a header, then uses positional values:

| Format | Tokens/issue | 50 issues |
| ------ | ------------ | --------- |
| XML    | ~70          | ~3,500    |
| JSON   | ~50          | ~2,500    |
| TOON   | ~25          | ~1,250    |

`--json` is available for scripts and pipelines.

## Install

### Homebrew (macOS & Linux)

```bash
brew tap frr149/lql
brew install lql
```

### Shell installer (macOS & Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/frr149/lql/releases/latest/download/lql-installer.sh | sh
```

### PowerShell installer (Windows)

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/frr149/lql/releases/latest/download/lql-installer.ps1 | iex"
```

### Windows MSI

Download from [GitHub Releases](https://github.com/frr149/lql/releases/latest).

### From source

```bash
cargo install --path .
# or
just install
```

### Update

If you installed with the shell/PowerShell installer, lql includes a built-in updater:

```bash
lql-update
```

### Requirements

A [1Password CLI](https://developer.1password.com/docs/cli/) setup with a Linear API key stored at `op://Private/Linear/api-key` (configurable in `~/.config/lql/config.toml`).

Alternatively, set the `LINEAR_API_KEY` environment variable to bypass 1Password (useful for CI/CD or frequent CLI usage):

```bash
# One-time setup (Fish shell)
set -gx LINEAR_API_KEY (op read "op://Private/Linear/api-key")

# Bash/Zsh
export LINEAR_API_KEY=$(op read "op://Private/Linear/api-key")
```

This reads the API key once per shell session instead of once per `lql` invocation.

## Usage

```bash
# List issues (auto-detects team from cwd)
lql list
lql list --team PROD --state Todo,started --priority high
lql list --overdue --all-teams

# Create
lql create "Fix auth token refresh" --project Tokamak --label tokamak --priority urgent
lql create "Write migration guide" --due friday -d "Include rollback steps"

# Update
lql update PROD-42 --state Done
lql update PROD-42 --priority urgent --label Bug

# View
lql view PROD-42

# Search
lql search "auth token" --team PROD

# Comment
lql comment PROD-42 "Investigated — the issue is in the token refresh logic"
echo "## Progress\nPartial fix deployed" | lql comment PROD-42

# Relations
lql relate PROD-42 blocks PROD-43
lql relate PROD-42 blocked-by PROD-41

# Diagnostics
lql doctor    # validate config, auth, API connectivity
lql context   # show resolved team/project/label for cwd
lql labels    # list all available labels

# Raw GraphQL (escape hatch)
lql raw '{ viewer { id name email } }'
```

When a label name exists in multiple teams, `lql` resolves it within the target team for `create`, `update`, and team-scoped `list`. This avoids sending a label ID from the wrong team.

## Configuration

`~/.config/lql/config.toml`:

```toml
[auth]
api_key_ref = "op://Private/Linear/api-key"

[defaults]
sort = "priority"
states = ["backlog", "unstarted", "started"]
limit = 50

[context-map]
"~/projects/tokamak" = { team = "PROD", project = "Tokamak", label = "tokamak" }
"~/code/myapp"   = { team = "PROD", project = "MyApp",   label = "myapp" }
"~/code/tools"   = { team = "TOOL" }

[state-aliases]
"Todo" = "unstarted"
"In Progress" = "started"
"Done" = "completed"
"Canceled" = "canceled"
"Cancelled" = "canceled"

[priority-aliases]
urgent = 1
high = 2
medium = 3
low = 4
none = 0

[retired-teams]
TOK = "Tokamak issues are now in PROD. Use: --team PROD --label tokamak"
```

The `context-map` auto-detects team, project, and label from your working directory. No flags needed when you're inside a project.

## Testing

```bash
just test          # 264 unit tests (no API calls)
just integration   # 9 integration tests against real Linear API
just all           # both
just lint          # clippy with warnings as errors
```

The test suite includes:

- 75 ERR test specifications from the PRD (64 unit, 9 integration, 1 deferred, 1 out of scope)
- Property-based tests with proptest (any casing of "Todo" normalizes to "unstarted")
- Real API response fixtures (captured from Linear, never generated)
- Mock-based tests via `GraphQLClient` trait (no API calls in CI)

## The methodology behind lql

lql was built using **adversarial programming** — a development methodology for AI-assisted coding where you assume your AI copilot will hallucinate APIs, invent flags, and take the most plausible-but-wrong path.

The key techniques:

1. **Schema-first development**: Download the real API schema before writing any code. Never let the AI guess field names.
2. **Real fixtures, never generated**: Every test fixture was captured from the real Linear API. The AI generates code against them, not the other way around.
3. **Two-layer validation (MDD)**: Layer 1 designs from anticipated errors (the PRD with 75 ERR specs). Layer 2 validates against real usage. [Layer 1 hit rate: 80%](docs/mdd_findings.md).
4. **Tolerance contract**: Adapt non-destructive input (normalize `--status` → `--state`). Reject destructive input (invented labels). Always inform what was assumed.
5. **Instruction elimination**: A tolerant tool needs fewer instructions. The Claude Code skill for Linear went from 246 lines (150 of workarounds) to 205 lines with zero workarounds.

### Read more

- [The wrong path should be impossible, not forbidden](https://frr.dev/en/the-wrong-path-should-be-impossible-not-forbidden/) — the core principle
- [Linear Agent is not what you need](https://frr.dev/en/linear-agent-cli-rust-agent-already-had/) — why we built lql instead of using Linear's built-in AI
- [Adversarial programming: when your AI copilot invents the API](https://frr.dev/en/adversarial-programming-ai-copilot-invents-api/) — schema-first defense against hallucinated APIs
- [150 lines of apologies eliminated](https://frr.dev/en/skill-before-after-tolerant-tool-fewer-instructions/) — how a tolerant tool erases defensive documentation
- [Why my CLI doesn't speak XML: TOON and tokens](https://frr.dev/en/why-my-cli-doesnt-speak-xml-toon-tokens/) — output format design for LLM consumers
- [MDD: Don Quixote and Sancho Panza as AI copilots](https://frr.dev/en/mdd-don-quixote-sancho-panza-ai-copilot/) — the two-layer validation methodology

## Cross-compile

```bash
just cross    # builds for x86_64-unknown-linux-musl
just deploy   # cross-compiles and scps to server
```

## License

MIT — see [LICENSE](LICENSE).

---

Built with Rust and [adversarial programming](https://frr.dev). By [Fernando Rodriguez Romero](https://frr.dev).
