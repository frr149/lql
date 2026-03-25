# CLAUDE.md — lql (Linear Query Language)

## Proyecto

CLI en Rust para gestionar issues de Linear. Reemplaza la CLI oficial (`linear`), el MCP de Linear, y `linear-curator`. Diseñada para ser consumida por LLMs (output compacto, zero flags obligatorios, tolerancia a errores de interfaz).

**PRD completo**: `docs/PRD.md`

## Comandos

```bash
cargo build                # compilar
cargo run -- list          # ejecutar un comando
cargo test                 # tests
cargo build --release      # release build
```

## Stack

- Rust (edición 2024)
- clap (derive) — CLI parsing con flag aliases
- reqwest (blocking) — HTTP para Linear API, OpenRouter, Telegram
- serde + serde_json — JSON (escapado correcto by construction)
- toml — config parsing
- fs2 — file locking (corrections.jsonl)

## Arquitectura

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

## Principios

1. **Linear API directo** — NUNCA usar la CLI `linear` ni el MCP. Solo GraphQL vía reqwest.
2. **serde para todo JSON** — NUNCA interpolar strings en queries GraphQL. Usar variables GraphQL + `serde_json::json!()`.
3. **Output compacto** — ~25 tokens/issue. Formato: `ID [State] labels — Title (age, due)`.
4. **Tolerancia de interfaz** — `--status` → `--state`, `Todo` → `unstarted`, `--priority urgent` → `--priority 1`. Normalizar, no rechazar.
5. **Sin cache en disco** — fetchear de Linear en cada ejecución (~200ms). Sin divergencia posible.
6. **Sin async** — todo blocking. Un CLI no necesita concurrencia interna.

## Config

Fichero: `~/.config/lql/config.toml`

Contiene:
- `[auth]` — referencia a 1Password para API key
- `[defaults]` — sort, states, limit
- `[context-map]` — directorio → team/project/label
- `[state-aliases]` — Todo→unstarted, Done→completed, etc.
- `[priority-aliases]` — urgent→1, high→2, etc.
- `[curator]` — LLM config para clasificación
- `[telegram]` — bot token y chat ID refs

Ver `docs/PRD.md` para el TOML completo.

## Datos locales

```
~/.local/share/lql/
└── corrections.jsonl    # few-shot examples para el clasificador (append-only)
```

Un solo fichero. Pending reviews viven en Linear (comentarios del curator), no en disco.

## Linear API

- Endpoint: `https://api.linear.app/graphql`
- Auth: `Authorization: <api-key>` (sin Bearer)
- API key: `op read "op://FRR DEV/Linear/api-key"`
- Rate limit: ~1500 req/h por key, compartido entre agentes
- Retry: exponential backoff en 429 (2s, 4s, 8s, max 3 retries)

## Cross-compile para wuwei

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
scp target/x86_64-unknown-linux-musl/release/lql wuwei.frr.dev:~/.local/bin/
```

## Fases

1. **Core CLI** — list, create, update, view, search, comment, relate, labels, doctor
2. **Curator + Review** — curate, review, summary, triage, Telegram
3. **Integración** — skill `/issues`, memento, Ansible role, archivar linear-curator
