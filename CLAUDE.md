# CLAUDE.md — lql (Linear Query Language)

## Project

Rust CLI for managing Linear issues. Designed for LLM consumption: compact output, zero mandatory flags, tolerant input normalization.

**Repo:** GitHub only (`origin` → `git@github.com:frr149/lql.git`). No Gitea mirror. All PRs go to GitHub.

## Commands

```bash
cargo build                # build
cargo run -- list          # run a command
cargo test                 # tests
cargo build --release      # release build
```

## Stack

- Rust (edition 2024)
- clap (derive) — CLI parsing with flag aliases
- reqwest (blocking) — HTTP for Linear API, OpenRouter, Telegram
- serde + serde_json — JSON (correct escaping by construction)
- toml — config parsing
- fs2 — file locking (corrections.jsonl)

## Architecture

```
src/
├── main.rs              # CLI entry point (clap App)
├── cli.rs               # Clap derive structs, flag aliases, normalization
├── config.rs            # TOML parsing, context-map resolution
├── client.rs            # GraphQL client (reqwest blocking + serde)
├── auth.rs              # op read wrapper
├── format.rs            # Output formatting (compact + JSON)
├── commands/
│   ├── list.rs
│   ├── create.rs
│   ├── update.rs
│   ├── view.rs
│   ├── search.rs
│   ├── comment.rs
│   ├── relate.rs
│   ├── labels.rs
│   ├── summary.rs
│   ├── triage.rs
│   ├── curate.rs        # LLM classification pipeline
│   ├── review.rs        # Resolve pending reviews
│   └── doctor.rs        # Validate config, auth, teams
├── curator/
│   ├── classifier.rs    # LLM batch classification (OpenRouter)
│   ├── corrections.rs   # Read/append corrections.jsonl
│   └── telegram.rs      # Digest notification
└── queries.rs           # GraphQL query/mutation constants
```

## Language

All user-facing text (CLI help, error messages, log messages) MUST be in English. Anything that appears in `--help`, `stderr`, or `stdout` is English only.

## Principles

1. **Direct Linear API** — Only GraphQL via reqwest. Never use the `linear` CLI or MCP.
2. **serde for all JSON** — Never interpolate strings into GraphQL queries. Use GraphQL variables + `serde_json::json!()`.
3. **Compact output** — ~25 tokens/issue. Format: `ID [State] labels — Title (age, due)`.
4. **Input tolerance** — `--status` → `--state`, `Todo` → `unstarted`, `--priority urgent` → `--priority 1`. Normalize, never reject.
5. **No disk cache** — Fetch from Linear on every run (~200ms). No divergence possible.
6. **No async** — All blocking. A CLI doesn't need internal concurrency.

## Config

File: `~/.config/lql/config.toml`

Sections:

- `[auth]` — 1Password reference for API key
- `[defaults]` — sort, states, limit
- `[context-map]` — directory → team/project/label
- `[state-aliases]` — Todo→unstarted, Done→completed, etc.
- `[priority-aliases]` — urgent→1, high→2, etc.
- `[curator]` — LLM config for classification
- `[telegram]` — bot token and chat ID refs

## Local data

```
~/.local/share/lql/
└── corrections.jsonl    # few-shot examples for the classifier (append-only)
```

Pending reviews live in Linear (curator comments), not on disk.

## Linear API

- Endpoint: `https://api.linear.app/graphql`
- Auth: `Authorization: <api-key>` (no Bearer prefix)
- API key: via 1Password (`op read`)
- Rate limit: ~1500 req/h per key
- Retry: exponential backoff on 429 (2s, 4s, 8s, max 3 retries)

## Cross-compile (Linux x86_64)

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

## Phases

1. **Core CLI** — list, create, update, view, search, comment, relate, labels, doctor
2. **Curator + Review** — curate, review, summary, triage, Telegram notifications
3. **Integration** — external tooling, deployment
