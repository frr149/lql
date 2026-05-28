# PRD: lql — CLI wrapper para Linear optimizado para LLMs

**Autor**: Fernando + Claude
**Fecha**: 2026-03-25
**Estado**: Borrador

## Problema

Claude Code interactúa con Linear ~800 veces al mes a través de la CLI oficial (`linear`) y la API GraphQL. Un análisis de 165 sesiones revela **500+ errores y 370+ reintentos** causados por:

1. **Flags inventados** (171 usos de MCP prohibido, 64 creates sin `--no-interactive`, 40+ lists sin `--sort`)
2. **Escapado JSON/GraphQL roto** (25+ fallos con 80+ reintentos en cadena)
3. **Confusión de nomenclatura** (`--status`/`--state`, `Todo`/`unstarted`, `urgent`/`1`)
4. **Operaciones imposibles por CLI** (filtro por label, search, asignar project)
5. **Output verbose** que desperdicia tokens del contexto del LLM

Cada error cuesta ~2000 tokens (comando + error + retry). Estimación conservadora: **700K tokens/mes desperdiciados** en interacción con Linear.

Note: The issue curation/triage workflow (previously `linear-curator`) has been extracted to a [separate project](https://github.com/frr149/curator) that composes with lql.

## Solución

`lql` ("Linear Query Language"): CLI en Rust que unifica tres cosas:

A CLI wrapper over the Linear GraphQL API with an interface designed for LLM agents and human power users.

No usa la CLI oficial — va directo a GraphQL, eliminando toda la capa de errores.

## Principios de diseño

### 1. Output token-efficient por defecto

El consumidor principal (95%) es un LLM (Claude Code). El LLM no parsea con regex — lee texto natural. Necesita: issue ID para follow-up, estado, contexto suficiente para decidir sin hacer `view`.

```
# CLI oficial (1 issue): ~55 tokens
▄▆█ PROD-587  Importar sesiones desde backup del NAS          qinqin  - -  [38;2;190;194;200mBacklog[39m [90m3 days ago[39m

# JSONL (1 issue): ~50 tokens (claves repetidas, peor que compacto para LLMs)
{"id":"PROD-587","state":"backlog","labels":["qinqin"],"title":"Importar sesiones...","age_days":14}

# lql (1 issue): ~25 tokens
PROD-587 [Backlog] qinqin — Importar sesiones desde backup del NAS (14d, overdue!)
```

Con 50 issues: CLI=2750 tokens, JSONL=2500, Compacto=1250. **Ratio 2:1 vs JSONL, 2.2:1 vs CLI.**

JSONL es peor que compacto para el LLM porque las claves (`"state":`, `"labels":`) se repiten en cada línea y el LLM no las necesita — entiende por posición. `--json` existe solo para scripts (memento, hooks).

### 2. Zero flags obligatorios

| CLI oficial                    | lql                 | Por qué                               |
| ------------------------------ | ------------------- | ------------------------------------- |
| `--sort priority` obligatorio  | Default `priority`  | El 100% de mis usos son por prioridad |
| `--no-pager` obligatorio       | Sin pager nunca     | LLMs no tienen terminal               |
| `--all-assignees` obligatorio  | Default all         | Solo hay un usuario                   |
| `--no-interactive` obligatorio | Nunca interactivo   | LLMs no pueden responder prompts      |
| `--team X` obligatorio         | Auto-detect por cwd | Ya tengo el context-map               |

Un `lql list` sin argumentos debe funcionar siempre.

### 3. Tolerancia total a errores de interfaz

Normalizaciones automáticas en el parser de argumentos:

| Input                   | Se normaliza a      | Error actual                |
| ----------------------- | ------------------- | --------------------------- |
| `--status Done`         | `--state completed` | `Unknown option "--status"` |
| `--state Todo`          | `--state unstarted` | `must be of type "state"`   |
| `--state "In Progress"` | `--state started`   | `must be of type "state"`   |
| `--state Done`          | `--state completed` | `must be of type "state"`   |
| `--state cancelled`     | `--state canceled`  | doble L                     |
| `--priority urgent`     | `--priority 1`      | `must be of type "number"`  |
| `--priority high`       | `--priority 2`      | ídem                        |
| `--priority medium`     | `--priority 3`      | ídem                        |
| `--priority low`        | `--priority 4`      | ídem                        |
| `--no-pager`            | (ignorado)          | `Unknown option`            |
| `--no-interactive`      | (ignorado)          | `Unknown option`            |
| `--sort updated`        | `--sort updatedAt`  | `must be of type "sort"`    |

Cualquier flag desconocido genera un mensaje **útil**, no críptico:

```
# CLI oficial:
error: Unknown option "--filter". Did you mean option "--state"?

# lql:
ERROR: --filter no existe. Para buscar por texto: lql search "texto"
       Para filtrar por estado: lql list --state backlog
       Para filtrar por label: lql list --label qinqin
```

### 4. Descripciones seguras por defecto

NUNCA escapado inline. Siempre `--description-file` o stdin:

```bash
# Aceptado:
lql create "título" --description-file /tmp/desc.md
lql create "título" <<'EOF'
Descripción con "comillas", `backticks` y $variables.
Todo funciona porque no hay shell escaping.
EOF

# También aceptado (string corta):
lql create "título" -d "Descripción simple sin markdown"
```

Para `-d` inline, `lql` escapa automáticamente al construir la mutación GraphQL. El LLM nunca ve el escapado.

## Formato de output

### Principio: un formato para LLM y humano, otro para scripts

El LLM no necesita claves JSON — entiende por posición. El humano no necesita ANSI codes — necesita scanning rápido. Ambos necesitan lo mismo: texto compacto, posicional, sin ruido. `--json` (JSONL) existe solo para scripts que parsean con jq/Python.

### `list` / `search` — una línea por issue

```
PROD-587 [Backlog] qinqin — Importar sesiones desde backup del NAS (14d, overdue!)
PROD-515 [Started] tokamak,bug — Extra usage detection sin API (3d)
PROD-529 [Backlog] wuwei — Mover media a Yottamaster (4d, due:Mar 21)
── 3 issues (1 overdue, 1 started, 1 backlog)
```

**Estructura**: `ID [State] labels — Title (age, due)`

| Elemento                  | Por qué ahí                                  | Ejemplo               |
| ------------------------- | -------------------------------------------- | --------------------- |
| ID primero                | El LLM lo necesita para el siguiente comando | `PROD-587`            |
| [State] en brackets       | Delimitador visual no ambiguo, posición fija | `[Backlog]`           |
| Labels sin brackets       | Separados por coma, no confundir con state   | `qinqin,bug`          |
| Em-dash separador         | No se confunde con guiones en títulos        | `—`                   |
| Metadata entre paréntesis | Age siempre, due solo si existe              | `(14d, overdue!)`     |
| Footer                    | Conteo por estado — ahorra al LLM contar     | `── 3 issues (...)` |

**Por qué no TSV**: el LLM lee texto, no tablas. Títulos largos rompen columnas fijas.

**`--json`** (solo para scripts):

```jsonl
{
  "id": "PROD-587",
  "state": "backlog",
  "labels": [
    "qinqin"
  ],
  "title": "Importar sesiones desde backup del NAS",
  "age_days": 14,
  "due": "2026-03-11",
  "overdue": true,
  "project": "Qinqin",
  "priority": 2
}
```

### `create` — confirmación + URL

```
✓ PROD-612 created [Todo] tokamak — Fix auth token refresh
  https://linear.app/frr149/issue/PROD-612
```

Una línea con todo lo que el LLM necesita (ID + estado). URL en segunda línea solo para `create` (el humano puede querer abrir).

### `update` — transición en una línea

```
✓ PROD-587 Backlog → Done
```

Transición visible. El LLM sabe que funcionó sin parsear nada. Sin URL (ya tiene el ID).

### `view` — compacto, descripción sin truncar

```
PROD-587 [Backlog] P2 qinqin — Importar sesiones desde backup del NAS
  Team: PROD | Project: Qinqin | Created: 14d | Due: overdue!
  ─────
  El NAS murió. Los discos están montados en wuwei como /mnt/keepcoding.
  Necesitamos importar las sesiones de Claude Code que estaban allí.
  ─────
  Relations: blocks PROD-588
  Comments: 2
```

Header = misma línea que `list` + prioridad. Descripción sin truncar (el LLM pidió `view` para leerla). Sin URL — el patrón es fijo: `linear.app/frr149/issue/{ID}`.

### Errores — accionables, con sugerencia

```
✗ Label "appstore" not found. Available: tokamak, qinqin, qualitra, blog, wuwei, ...
✗ PROD-999 not found. Similar: PROD-599 "OAuth token refresh"
```

Sin stack traces. Si un valor se puede normalizar automáticamente, no es error — es corrección silenciosa con log a stderr:

```
(stderr) ℹ State "Todo" → normalized to "unstarted"
```

El LLM ve que funcionó en stdout y aprende el nombre correcto en stderr. Sin retry necesario.

## Comandos

### `lql list` — Listar issues

```bash
# Auto-detect team del cwd, estados activos, sort priority
lql list

# Filtrar por label (EL CASO DE USO #1 QUE FALTA)
lql list --label qinqin
lql list --label tokamak --label bug

# Filtrar por project
lql list --project Tokamak

# Filtrar por estado
lql list --state backlog
lql list --state started,unstarted   # múltiples con coma

# Combinar filtros
lql list --label qinqin --state backlog --project Qinqin

# Todos los teams
lql list --all-teams

# Issues vencidas
lql list --overdue

# Todos los resultados (sin límite)
lql list --all

# Output JSON (para pipe/parsing por scripts, NO para LLMs)
lql list --json
```

**Paginación**: por defecto `limit = 50` (configurable en TOML). Cuando hay más resultados, el footer lo indica:

```
── showing 50 of 87 issues (use --all or --limit N for more)
```

### `lql create` — Crear issue

```bash
# Mínimo (auto-detect team, project, label del cwd)
lql create "Fix auth token refresh"

# Con descripción
lql create "Fix auth token refresh" --description-file /tmp/desc.md

# Con descripción inline (escapado automático)
lql create "Fix auth token refresh" -d "El token OAuth expira y no se refresca"

# Con stdin
lql create "Fix auth token refresh" <<'EOF'
## Problema
El token OAuth expira tras 8h y no se refresca automáticamente.

## Solución propuesta
Detectar `expiresAt` < now + 5min y emitir warning en logs.
EOF

# Override team/project/label
lql create "Fix auth" --team CONT --project Blog --label blog

# Con prioridad (acepta número O nombre)
lql create "Fix auth" --priority urgent   # se normaliza a 1
lql create "Fix auth" --priority 1        # también funciona

# Con due date
lql create "Fix auth" --due 2026-04-01
lql create "Fix auth" --due friday        # fecha relativa
lql create "Fix auth" --due +7d           # en 7 días
```

**Detección de duplicados** (antes de crear):

```
⚠ Issues similares encontradas:
  PROD-602 [Backlog] "OAuth token refresh — detect expiry" (85% match)
  PROD-511 [Done] "Fix OAuth token handling" (62% match)

Creando de todos modos. Usa --force para omitir esta comprobación.
```

La detección usa `searchIssues(term:)` con el título. Si hay match >70% en issues activas, avisa en stderr. `--force` omite la comprobación. En modo no-TTY (LLM), emite warning y crea — el LLM decide si abortar.

### `lql update` — Actualizar issue

```bash
# Cambiar estado (acepta UI names)
lql update PROD-587 --state Done        # → completed
lql update PROD-587 --state started

# Cambiar prioridad
lql update PROD-587 --priority urgent   # → 1

# Mover de proyecto
lql update PROD-587 --project Tokamak

# Añadir label
lql update PROD-587 --label bug

# Cambiar título
lql update PROD-587 --title "Nuevo título"

# Actualizar descripción
lql update PROD-587 --description-file /tmp/desc.md
```

### `lql view` — Ver detalle de issue

```bash
lql view PROD-587
```

### `lql search` — Búsqueda por texto

```bash
lql search "basedpyright"
lql search "OAuth token" --team PROD --state backlog,unstarted
```

Usa `searchIssues(term:)` de la API. Output idéntico a `list`.

### `lql summary` — Resumen ejecutivo

```bash
lql summary                # team auto-detectado
lql summary --all-teams    # global
```

### `lql comment` — Añadir comentario

```bash
# Inline
lql comment PROD-587 "Investigado, el problema es X"

# Desde archivo
lql comment PROD-587 --file /tmp/comment.md

# Desde stdin
lql comment PROD-587 <<'EOF'
## Progreso
- [x] Investigar root cause
- [ ] Implementar fix
EOF
```

### `lql relate` — Crear relaciones

```bash
lql relate PROD-587 blocks PROD-588
lql relate PROD-587 blocked-by PROD-515
lql relate PROD-587 related PROD-520
```

Normaliza `blocked-by` → `blocks` con issue invertida.

### `lql labels` — Listar labels disponibles

```bash
lql labels
lql labels --team PROD
```

Para que el LLM nunca invente labels.

### `lql raw` — Escape hatch para queries GraphQL arbitrarias

Cuando `lql` no cubre una operación, `raw` permite ejecutar queries GraphQL directas sin salir del wrapper (evitando el fallback a `curl` + escapado manual que causa 80+ reintentos).

```bash
# Query inline
lql raw 'query { viewer { id name } }'

# Desde fichero (para mutations complejas)
lql raw --file query.graphql

# Con variables
lql raw --file query.graphql --var teamId=PROD-UUID --var title="Fix bug"

# Variables desde fichero JSON
lql raw --file query.graphql --vars-file vars.json
```

**Comportamiento**:

- Output: JSON crudo sin formatear (lo que devuelve Linear)
- Auth: usa la misma API key que el resto de lql
- Variables: `--var key=value` repetible, se construyen como objeto JSON con serde (escapado seguro)
- Sin normalización ni aliases — el usuario escribe GraphQL puro
- Exit 1 si la API devuelve errores, con el mensaje de error de Linear

**No incluye**: introspección de schema, autocompletado de campos, formateo del resultado. Es un pipe directo al endpoint GraphQL con auth y serde.

### ~~Curator commands (extracted to separate project)~~

> `curate`, `review`, `triage`, and `summary` have been extracted to [curator](https://github.com/frr149/curator), a separate Python project that composes with lql via `lql list --no-label --json` and `lql update --label`. This follows Unix philosophy: lql handles Linear API interaction, curator handles LLM classification and Telegram review UX. See the [curator PRD](https://github.com/frr149/curator/blob/main/docs/PRD.md).

## Configuración

### Archivo `~/.config/lql/config.toml`

```toml
[auth]
api_key_ref = "op://<your-vault>/Linear/api-key"  # leído via op read (con cache)

[defaults]
sort = "priority"        # default para list
states = ["backlog", "unstarted", "started"]  # excluye completed/canceled
limit = 50

[context-map]
# directorio → team, project, label
"~/code/tokamak"         = { team = "PROD", project = "Tokamak",    label = "tokamak" }
"~/code/qinqin"          = { team = "PROD", project = "Qinqin",     label = "qinqin" }
"~/code/qualitra"        = { team = "PROD", project = "Qualitra",   label = "qualitra" }
"~/code/frr.dev"         = { team = "CONT", project = "Blog",       label = "blog" }
"~/code/wuwei"           = { team = "PRIV", label = "wuwei" }
"~/code/kc-raven"        = { team = "KC",   label = "kc_raven" }
"~/code/rustyclaw"       = { team = "PROD", project = "RustyClaw",  label = "rustyclaw" }
"~/code/social-publisher" = { team = "TOOL", project = "Social Publisher", label = "workflows" }
"~/code/auto_correct"    = { team = "TOOL", project = "auto_correct", label = "autocorrect" }
"~/code/memento"         = { team = "TOOL", project = "memento",    label = "workflows" }
"~/code/mcp-email"       = { team = "TOOL", label = "claude-code" }
"~/code/lql"             = { team = "TOOL", label = "lql" }

[state-aliases]
# UI name → CLI value
"Todo"        = "unstarted"
"In Progress" = "started"
"Done"        = "completed"
"Canceled"    = "canceled"
"Cancelled"   = "canceled"

[priority-aliases]
"urgent"  = 1
"high"    = 2
"medium"  = 3
"low"     = 4
"none"    = 0

[flag-aliases]
# Flags incorrectos comunes → corrección
"--status"         = "--state"
"--filter"         = "--search"    # sugiere comando correcto
"--no-limit"       = "--limit 0"
"--query"          = "--search"
"--relates-to"     = "--relate"
"--comment"        = "→ lql comment"

# [curator] and [telegram] config moved to curator project
# See: https://github.com/frr149/curator
```

## Stack técnico

- **Rust** (cargo project, edición 2024)
- **clap** (derive) para CLI — flag aliases en compile time (`#[arg(alias = "status")]`)
- **reqwest** (blocking, rustls-tls) para HTTP — Linear API
- **serde** + **serde_json** para JSON — escapado correcto by construction, el problema #1 (80+ reintentos) es imposible
- **chrono** para fechas — due dates relativos (`--due friday`, `--due +7d`), cálculo de age/overdue
- **toml** para config
- **Auth**: `op read` vía `std::process::Command` (usa el wrapper cache de `~/.local/bin/op`)
- **Sin async**: todo blocking. Un CLI no necesita concurrencia interna.

### Por qué Rust y no Python

|                           | Python                               | Rust                                                                                     |
| ------------------------- | ------------------------------------ | ---------------------------------------------------------------------------------------- |
| **Startup**               | ~50ms (import overhead)              | <1ms                                                                                     |
| **Deploy**                | Clonar repo + `uv sync` + `.venv`    | Copiar un binario                                                                        |
| **JSON escaping**         | `json.dumps` (correcto pero runtime) | `serde_json::to_value` (correcto by construction)                                        |
| **Flag validation**       | Runtime (click)                      | Compile time (clap derive)                                                               |
| **Cross-compile**         | No aplica                            | `cargo build --target x86_64-unknown-linux-musl` desde Mac → binario estático para wuwei |
| **Dependencies en wuwei** | Python 3.12+, uv, venv, git clone    | Nada. Un binario estático.                                                               |

### Escapado seguro — la solución al problema #1

```rust
// serde_json NUNCA produce JSON roto.
// Cualquier string (markdown, comillas, backticks, newlines) se escapa correctamente.

let variables = serde_json::json!({
    "input": {
        "title": title,           // escapado automático
        "description": desc,      // cualquier markdown funciona
        "teamId": team_id,
        "stateId": state_id,
        "labelIds": label_ids,
        "projectId": project_id,
    }
});

let body = serde_json::json!({
    "query": ISSUE_CREATE_MUTATION,  // const &str, query fija
    "variables": variables,
});

// reqwest serializa body → JSON perfecto, siempre.
client.post(GRAPHQL_URL)
    .header("Authorization", &api_key)
    .json(&body)
    .send()?;
```

Esto elimina el 100% de los errores de escapado. No hay interpolación de strings, no hay shell piping, no hay jq. El compilador garantiza que el JSON es válido.

### Datos persistentes

```
~/.local/share/lql/
└── corrections.jsonl        # few-shot examples para el clasificador
```

Un solo fichero local. No es estado de la app — es un dataset de entrenamiento para el prompt del clasificador LLM.

```jsonl
{
  "issue": "PROD-618",
  "title": "Configurar cron nocturno en wuwei",
  "curator_said": "blog",
  "user_corrected": "wuwei",
  "reason": "infra task, not content",
  "timestamp": "2026-03-10T..."
}
```

- **Quién escribe**: `review` (append con `fs2::FileExt::lock_exclusive` cuando el usuario corrige una sugerencia)
- **Quién lee**: `curate` (snapshot de las últimas 20 correcciones al inicio, inyectadas como few-shot en el system prompt)
- **Concurrencia**: semántica de log — append + snapshot. Sin conflicto.
- **Crecimiento**: ~10 correcciones/mes. El fichero tendrá ~100 líneas en un año.

### Pending reviews: viven en Linear, no en disco

Las issues pendientes de revisión NO se persisten localmente. Una issue sin label de taxonomía **es** una pending review — Linear es la fuente de verdad.

`curate` deja la sugerencia como **comentario en la issue**:

```
🏷 Curator suggestion: blog (71%) — "mentions pipeline and cron"
```

`review` lee ese comentario para mostrar la sugerencia. Si el usuario aplica el label (vía `review --accept` o `review --label X`), la issue desaparece del pending porque ya tiene label. El comentario queda como audit trail.

Esto elimina `pending-reviews.jsonl` y con él todo riesgo de concurrencia entre `curate` y `review`.

### Sin cache en disco

Labels, teams y projects se fetchean de Linear en cada ejecución (~200ms). No hay cache entre procesos. Razón: cualquier cache en disco puede divergir de Linear (el usuario crea un label en la UI, el cache no lo tiene, `create --label nuevo` rechaza la validación local → falso negativo que impide trabajar). El coste de 200ms extra no justifica ese riesgo.

Dentro de una misma ejecución (especialmente `curate`, que procesa ~100 issues), la metadata se cachea en memoria del proceso. Pero muere con él.

### Multi-agent safe by design

Múltiples agentes (Claude Code, Codex, otros) pueden usar `lql` concurrentemente sin coordinación:

- **Sin cache en disco** → no hay estado compartido que pueda divergir
- **API stateless** → cada proceso hace su propia request, Linear resuelve concurrencia server-side
- **Context por cwd** → cada agente detecta su team/project independientemente
- **Rate limit compartido** → todos usan la misma API key (~1500 req/h). Con 2-3 agentes activos no es un problema, pero 429 se maneja con retry + exponential backoff (2s, 4s, 8s, max 3 retries)
- **Pending reviews en Linear** (comentarios) → `curate` escribe comentarios, `review` los lee. Concurrencia resuelta por Linear server-side. Sin fichero compartido.
- **`corrections.jsonl`** (único fichero local) → append-only por `review`, snapshot-read por `curate`. Semántica de log, sin conflicto. `fcntl.flock` para appends por higiene.

## Instalación

```bash
# Desarrollo (Mac)
cd ~/code/lql
cargo build --release
cp target/release/lql ~/.local/bin/

# O con cargo install
cargo install --path .
```

## Cross-compile para wuwei

wuwei es x86_64 Linux. Desde Mac:

```bash
# Una vez: instalar target
rustup target add x86_64-unknown-linux-gnu

# Build estático (musl para zero runtime deps)
cargo build --release --target x86_64-unknown-linux-musl

# Copiar a wuwei
scp target/x86_64-unknown-linux-musl/release/lql wuwei.frr.dev:~/.local/bin/
```

Sin git clone, sin venv, sin runtime. Un binario.

## Despliegue nocturno (wuwei)

Reemplaza el systemd timer de `linear-curator`:

```ini
# ~/.config/systemd/user/lql-curate.service
[Unit]
Description=LQL — nightly curation

[Service]
Type=oneshot
ExecStart=/home/admin_user/.local/bin/lql curate
TimeoutStartSec=300
EnvironmentFile=/home/admin_user/.config/lql/.env.local
```

```ini
# ~/.config/systemd/user/lql-curate.timer
[Unit]
Description=LQL — nightly curation timer

[Timer]
OnCalendar=*-*-* 03:30:00
Persistent=true
RandomizedDelaySec=120

[Install]
WantedBy=timers.target
```

El role de Ansible se simplifica: ya no clona repos ni instala Python. Solo copia el binario cross-compilado, despliega config y `.env.local`, y habilita el timer.

## Integración con Claude Code

### Cambios en CLAUDE.md global

```markdown
## Linear

- **CLI**: `lql` (wrapper custom). NUNCA usar `linear` (CLI oficial) ni MCP ni curl a api.linear.app.
- Para gestionar issues, usar el skill `/issues` que delega en `lql`.
```

### Cambios en skill `/issues`

El skill se simplifica drásticamente. Ya no necesita documentar flags, estados, workarounds, GraphQL queries, team IDs, project IDs, ni label taxonomía. Todo eso vive en lql. El skill pasa a ser:

```
# SIEMPRE usar lql, NUNCA linear CLI ni curl a api.linear.app
lql <command> [args]

# Ejemplos:
lql list                              # auto-detect team del cwd
lql list --label qinqin               # filtrar por label
lql create "título" -d "desc"         # auto team/project/label
lql update PROD-587 --state Done      # acepta UI names
lql search "basedpyright"             # búsqueda por texto
lql summary                           # resumen ejecutivo
lql review                            # resolver pendientes del curator
lql triage                            # vista unificada de higiene
```

El context-map, los aliases de estado/prioridad, y la taxonomía de labels viven en `~/.config/lql/config.toml`. Claude Code no necesita saber nada de eso.

### Cambios en `memento`

`memento` delega en `lql triage --all-teams` en vez de implementar su propia query GraphQL. Reduce el código de memento y centraliza la lógica.

### Retiro de `linear-curator`

linear-curator queda absorbido. El repo se archiva. El role de Ansible se actualiza para apuntar a lql.

## Métricas de éxito

| Métrica                                      | Antes                 | Objetivo       |
| -------------------------------------------- | --------------------- | -------------- |
| Errores Linear por sesión                    | ~3                    | 0              |
| Reintentos por error                         | ~2.5                  | 0              |
| Tokens por `list` de 50 issues               | ~7500                 | ~2000          |
| Fallbacks a GraphQL manual                   | ~30% de sesiones      | 0%             |
| Labels inventados                            | ~10/mes               | 0              |
| Uso de MCP Linear                            | ~20/mes               | 0              |
| Issues sin label (curator pending) resueltas | días                  | minutos        |
| Servicios nocturnos desplegados              | 2 (memento + curator) | 1 (lql curate) |

## Estructura del proyecto

```
lql/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point (clap App)
│   ├── cli.rs               # Clap derive structs, flag aliases, normalization
│   ├── config.rs            # TOML parsing, context-map resolution
│   ├── client.rs            # GraphQL client (reqwest blocking + serde)
│   ├── auth.rs              # op read wrapper
│   ├── format.rs            # Output formatting (compact + JSON)
│   ├── commands/
│   │   ├── list.rs
│   │   ├── create.rs
│   │   ├── update.rs
│   │   ├── view.rs
│   │   ├── search.rs
│   │   ├── comment.rs
│   │   ├── relate.rs
│   │   ├── labels.rs
│   │   ├── raw.rs              # Escape hatch: GraphQL directo
│   │   ├── summary.rs
│   │   ├── triage.rs
│   │   ├── curate.rs        # LLM classification pipeline
│   │   ├── review.rs        # Resolve pending reviews
│   │   └── doctor.rs        # Validate config, auth, teams
│   ├── curator/
│   │   ├── classifier.rs    # LLM batch classification (OpenRouter)
│   │   ├── corrections.rs   # Read/append corrections.jsonl
│   │   └── telegram.rs      # Digest notification
│   └── queries.rs           # GraphQL query/mutation constants
├── tests/
│   ├── format_test.rs       # Output format compliance
│   ├── normalize_test.rs    # State/priority/flag alias tests
│   └── config_test.rs       # TOML parsing, context-map
└── config.example.toml
```

## Fases

### Fase 1 — Core CLI

- `list`, `create`, `update`, `view`, `search`, `comment`, `relate`, `labels`, `raw`
- Config TOML con context-map, state/priority aliases
- Normalización automática de toda la interfaz (clap aliases + normalize layer)
- GraphQL client con serde (escapado correcto by construction)
- Output compacto + `--json`
- Detección de duplicados en `create`
- `lql doctor` — validar config, auth, teams, labels
- Tests de formato y normalización

**Note:** Curator, review, triage, and summary functionality has been extracted to a separate project: [curator](https://github.com/frr149/curator). lql provides the CLI interface that curator composes with via `lql list --json` and `lql update`.

## Recomendaciones de producto y recorte

Sección añadida a partir de una revisión crítica del PRD desde la perspectiva de uso real por agentes como Codex. La dirección general del proyecto es buena y claramente mejor que la CLI oficial para consumo por LLMs, pero conviene introducir algunos ajustes para evitar sobrediseño en la primera iteración.

### Sugerencias de diseño

#### 1. Añadir `lql context`

El auto-detect por cwd es una de las mayores ventajas del diseño, pero necesita una forma explícita de inspección. Propuesta:

```bash
lql context
```

Output:

```text
Context: /Users/fernando/code/tokamak
  Team: PROD
  Project: Tokamak
  Label: tokamak
  Source: ~/.config/lql/config.toml
```

Esto reduce ambigüedad para humanos y agentes y facilita debug cuando un cwd resuelve a un team/project inesperado.

#### 2. Hacer estructuradas las sugerencias del curator

La idea de usar comentarios en Linear como fuente de verdad es buena, pero no conviene depender de texto libre como protocolo. En vez de parsear solo:

```text
🏷 Curator suggestion: blog (71%) — "mentions pipeline and cron"
```

conviene incluir una carga estructurada mínima y versionada dentro del comentario, por ejemplo:

```text
🏷 Curator suggestion: blog (71%) — "mentions pipeline and cron"
<!-- lql:review {"version":1,"suggested":"blog","confidence":0.71} -->
```

Así `review` puede leer un marcador estable aunque cambie el texto visible del comentario.

#### 3. Mantener un escape hatch deliberado

Aunque el objetivo sea cubrir el 95% de casos comunes, Linear tiene mucha superficie y siempre aparecerán operaciones raras. Conviene prever desde el principio una salida controlada, por ejemplo:

```bash
lql raw query.graphql --var team=PROD
```

o un subcomando equivalente solo para usuarios avanzados. No debe ser el camino principal, pero evita bloquear el uso cuando el wrapper todavía no cubre algo.

#### 4. `--create-label` debe ser excepcional

El PRD contempla `--create-label` como escape cuando un label no existe. Para agentes esto es peligroso porque facilita crear taxonomía basura por error semántico. Recomendación:

- no incluir `--create-label` en el MVP
- si se implementa después, exigir `--team` explícito o confirmación adicional
- limitarlo a labels team-specific, nunca workspace-level por defecto

#### 5. Separar claramente output humano-LLM de output estable para automatización

El formato compacto propuesto es correcto para agentes y terminal, pero scripts y hooks necesitarán contratos más rígidos. Recomendación:

- conservar el formato compacto como default
- mantener `--json` como JSONL estable
- documentar explícitamente qué campos están garantizados en JSON y su compatibilidad semántica entre versiones

#### 6. Añadir una noción explícita de límites del comando

`lql list` sin argumentos debe funcionar siempre, sí, pero también debe ser acotado. Conviene fijar desde el principio:

- límite por defecto visible (`limit = 50`)
- footer con `showing 50 of N` cuando haya más resultados
- opción clara para paginar o pedir todo (`--limit`, `--all`)

Sin esto, el ahorro de tokens puede degradarse rápido en equipos con muchos issues activas.

### Propuesta de MVP

El MVP debe demostrar que `lql` ya sustituye a la CLI oficial en el trabajo diario de agentes y humanos, pero sin absorber todavía todo `linear-curator`.

#### Objetivo del MVP

Cubrir el bucle operativo principal:

1. descubrir issues
2. leer contexto suficiente
3. crear y actualizar issues sin fricción
4. comentar y relacionar issues
5. validar configuración y contexto

#### Alcance del MVP

- `list`
- `view`
- `search`
- `create`
- `update`
- `comment`
- `relate`
- `labels`
- `doctor`
- `context`
- `raw` (escape hatch para GraphQL directo)
- Config TOML con `context-map`, aliases de estado y prioridad
- Output compacto por defecto + `--json`
- Validación fuerte de team/project/label
- Detección de duplicados en `create`
- GraphQL client directo con variables y serde
- Tests de normalización, formato y errores de auth/config

#### Fuera del MVP

- `curate`
- `review`
- `summary`
- `triage`
- Telegram
- feedback loop con `corrections.jsonl`
- sustitución de `memento`
- despliegue nocturno en `wuwei`
- `--create-label`

#### Criterio de “MVP listo”

Se puede cambiar el skill `/issues` para usar `lql` por defecto en trabajo diario sin recurrir a `linear` CLI ni a GraphQL manual en los casos habituales.

### Extracted: curator project

The v2.0 scope (curate, review, triage, summary, Telegram, corrections.jsonl) has been extracted to [curator](https://github.com/frr149/curator), a separate Python project. This follows Unix philosophy: lql handles Linear API interaction, curator handles LLM classification and user review UX. lql's scope is the core CLI — it does one thing well.

## Edge cases detectados en la auditoría

Errores reales que el diseño inicial no contemplaba explícitamente:

### 1. Label no encontrado — ¿crear o rechazar?

Cuando `lql create --label appstore` y el label no existe en Linear:

- **MVP**: rechazar con error claro + listar labels similares
- **v2.0**: `--create-label` con `--team` obligatorio (ver sección v2.0)

```
✗ Label "appstore" not found.
  Similar: tokamak, autocorrect
  Available: tokamak, qinqin, qualitra, blog, wuwei, ...
```

Razón: 10+ errores por labels inventados. Rechazar por defecto previene labels basura.

### 2. `op read` falla — mensaje claro

Cuando 1Password no puede leer la API key (Touch ID dismissed, timeout, no session):

```
✗ Could not read API key from 1Password.
  Run: op read "op://<your-vault>/Linear/api-key"
  If this fails, check: op signin
```

No un stack trace de `op`. Un mensaje que dice qué hacer.

### 3. Project name matching — case insensitive

`--project qinqin` debe encontrar "Qinqin". `--project "social publisher"` debe encontrar "Social Publisher". Matching case-insensitive + trim en el resolver de nombres.

Razón: 1 error real por `Project "qinqin" not found. Similar projects: Qinqin`.

## Apéndice: Catálogo completo de errores que lql elimina

| #   | Error                                  | Ocurrencias       | Cómo lo elimina lql               |
| --- | -------------------------------------- | ----------------- | --------------------------------- |
| 1   | `--sort` olvidado                      | 40+               | Default `priority`                |
| 2   | `--no-pager` en create/update          | 15+               | Sin pager nunca                   |
| 3   | `--status` vs `--state`                | 11+               | Flag alias automático             |
| 4   | `Todo`/`Done`/`In Progress`            | 12+               | State alias automático            |
| 5   | `--priority urgent` (string)           | 17+               | Priority alias automático         |
| 6   | `--no-interactive` ausente             | 64                | Nunca interactivo                 |
| 7   | `--comment` en update                  | 11                | Subcomando `comment` separado     |
| 8   | Labels inventados                      | 10+               | `labels` cmd + validación         |
| 9   | Team retirado (TOK)                    | 97+               | Context-map, no teams retirados   |
| 10  | Project IDs inventados                 | 9                 | Project por nombre, UUID resuelto |
| 11  | Escapado JSON/GraphQL                  | 25+ (80+ retries) | Variables GraphQL                 |
| 12  | `KeyError: 'data'`                     | 15+               | Error handling en respuesta       |
| 13  | Auth sin `&&`                          | 5+                | httpx directo, no shell piping    |
| 14  | jq parsing failures                    | 6+                | Sin jq, parseo nativo             |
| 15  | Campos GraphQL inventados              | 10+               | Queries fijas y testeadas         |
| 16  | MCP Linear                             | 171               | No existe, solo lql               |
| 17  | Cascada parallel calls                 | 4+                | Operaciones atómicas              |
| 18  | `--team` olvidado                      | 5                 | Auto-detect por cwd               |
| 19  | `--filter`/`--query`/`--label` en list | 6                 | Flags nativos                     |
| 20  | `linear search` (no existe)            | 1                 | `lql search`                      |

**Total eliminado: 500+ errores/mes → 0**

## Apéndice: Casos de test derivados de errores reales

Cada caso es un error real observado en sesiones de Claude Code. El ID de test sigue el formato `ERR-XX`. Convertir a tests de integración y/o unit tests.

### CLI — Normalización de flags

```
ERR-01: `lql list` sin --sort debe devolver resultados ordenados por prioridad (default)
  Input:  lql list --team PROD
  Expect: output ordenado por prioridad, sin error

ERR-02: `--status` se normaliza a `--state`
  Input:  lql update PROD-587 --status Done
  Expect: normaliza a --state completed, aplica, stderr: "ℹ --status → normalized to --state"

ERR-03: `--state Todo` se normaliza a `--state unstarted`
  Input:  lql list --state Todo
  Expect: filtra por unstarted, stderr: "ℹ State "Todo" → normalized to "unstarted""

ERR-04: `--state "In Progress"` se normaliza a `--state started`
  Input:  lql list --state "In Progress"
  Expect: filtra por started

ERR-05: `--state Done` se normaliza a `--state completed`
  Input:  lql update PROD-587 --state Done
  Expect: actualiza a completed

ERR-06: `--state cancelled` (doble L) se normaliza a `--state canceled`
  Input:  lql update PROD-587 --state cancelled
  Expect: actualiza a canceled

ERR-07: `--priority urgent` se normaliza a `--priority 1`
  Input:  lql create "Fix bug" --priority urgent
  Expect: crea con priority 1, stderr: "ℹ Priority "urgent" → normalized to 1"

ERR-08: `--priority high` se normaliza a `--priority 2`
  Input:  lql create "Fix bug" --priority high
  Expect: crea con priority 2

ERR-09: `--priority medium` se normaliza a `--priority 3`
  Input:  lql create "Fix bug" --priority medium
  Expect: crea con priority 3

ERR-10: `--priority low` se normaliza a `--priority 4`
  Input:  lql create "Fix bug" --priority low
  Expect: crea con priority 4

ERR-11: `--no-pager` se ignora silenciosamente en cualquier comando
  Input:  lql create "Fix bug" --no-pager
  Expect: crea sin error (flag ignorado)

ERR-12: `--no-interactive` se ignora silenciosamente
  Input:  lql create "Fix bug" --no-interactive
  Expect: crea sin error (flag ignorado)

ERR-13: `--sort updated` se normaliza a `--sort updatedAt`
  Input:  lql list --sort updated
  Expect: lista ordenada por updatedAt
```

### CLI — Flags inexistentes con mensaje útil

```
ERR-14: `--filter` da mensaje útil
  Input:  lql list --filter "backlog"
  Expect: exit 1, stderr: "✗ --filter no existe. Para filtrar por estado: --state backlog. Para buscar: lql search \"texto\""

ERR-15: `--query` da mensaje útil
  Input:  lql list --query "basedpyright"
  Expect: exit 1, stderr: "✗ --query no existe. Usa: lql search \"basedpyright\""

ERR-16: `--no-limit` da mensaje útil
  Input:  lql list --no-limit
  Expect: exit 1, stderr: "✗ --no-limit no existe. Usa: --limit 0"

ERR-17: `--relates-to` da mensaje útil
  Input:  lql update PROD-587 --relates-to PROD-588
  Expect: exit 1, stderr: "✗ --relates-to no existe. Usa: lql relate PROD-587 related PROD-588"

ERR-18: `--comment` en update da mensaje útil
  Input:  lql update PROD-587 --comment "texto"
  Expect: exit 1, stderr: "✗ --comment no existe en update. Usa: lql comment PROD-587 \"texto\""
```

### CLI — Context detection

```
ERR-19: auto-detect team desde cwd ~/code/tokamak
  Input:  cd ~/code/tokamak && lql list
  Expect: lista issues de team PROD con label tokamak

ERR-20: auto-detect team desde cwd ~/code/qinqin
  Input:  cd ~/code/qinqin && lql list
  Expect: lista issues de team PROD con label qinqin

ERR-21: cwd sin match en context-map → pide team explícito
  Input:  cd /tmp && lql list
  Expect: exit 1, stderr: "✗ Could not detect team from /tmp. Use --team <TEAM>. Available: PROD, CONT, PRIV, TOOL, KC"

ERR-22: --team override tiene prioridad sobre cwd
  Input:  cd ~/code/tokamak && lql list --team CONT
  Expect: lista issues de team CONT (no PROD)
```

### Labels — Validación

```
ERR-23: label inexistente rechazado con sugerencias
  Input:  lql create "Fix bug" --label appstore
  Expect: exit 1, stderr: "✗ Label \"appstore\" not found. Similar: tokamak, autocorrect. Use --create-label to create it."

ERR-24: label "enhancement" inexistente rechazado
  Input:  lql create "Fix bug" --label enhancement
  Expect: exit 1, stderr incluye labels disponibles

ERR-25: label "qa" inexistente rechazado
  Input:  lql create "Fix bug" --label qa
  Expect: exit 1, stderr incluye labels disponibles

ERR-26: label "infra" inexistente rechazado
  Input:  lql create "Fix bug" --label infra
  Expect: exit 1, stderr incluye labels disponibles

ERR-27: label existente funciona sin error
  Input:  lql create "Fix bug" --label tokamak
  Expect: crea con label tokamak

```

> **Nota**: ERR-28 (`--create-label`) queda fuera del MVP. Ver sección v2.0.

### Projects — Resolución por nombre

```
ERR-29: project por nombre exacto
  Input:  lql create "Fix bug" --project Tokamak
  Expect: asigna project Tokamak (UUID resuelto internamente)

ERR-30: project por nombre case-insensitive
  Input:  lql create "Fix bug" --project qinqin
  Expect: asigna project Qinqin

ERR-31: project por nombre con espacios case-insensitive
  Input:  lql create "Fix bug" --project "social publisher"
  Expect: asigna project "Social Publisher"

ERR-32: project inexistente rechazado con sugerencias
  Input:  lql create "Fix bug" --project Dashboard
  Expect: exit 1, stderr: "✗ Project \"Dashboard\" not found. Available: Tokamak, Qinqin, ..."

ERR-33: project ID numérico rechazado
  Input:  lql create "Fix bug" --project 686615456359
  Expect: exit 1, stderr: "✗ Use project name, not ID. Available: Tokamak, Qinqin, ..."
```

### Teams — Teams retirados

```
ERR-34: --team TOK rechazado
  Input:  lql list --team TOK
  Expect: exit 1, stderr: "✗ Team TOK is retired. Tokamak issues are now in PROD. Use: --team PROD --label tokamak"

ERR-35: --team QIN rechazado
  Input:  lql list --team QIN
  Expect: exit 1, stderr: "✗ Team QIN is retired. Use: --team PROD --label qinqin"

ERR-36: --team BLO rechazado
  Input:  lql list --team BLO
  Expect: exit 1, stderr: "✗ Team BLO does not exist. Did you mean: CONT?"

ERR-37: --team PER rechazado
  Input:  lql list --team PER
  Expect: exit 1, stderr: "✗ Team PER does not exist. Did you mean: PRIV?"

ERR-38: --team BLOG rechazado
  Input:  lql list --team BLOG
  Expect: exit 1, stderr: "✗ Team BLOG does not exist. Did you mean: CONT?"
```

### Escapado — Descripciones con caracteres especiales

```
ERR-39: descripción con comillas dobles
  Input:  lql create "Fix bug" -d 'El campo "title" no se escapa'
  Expect: issue creada, descripción preservada literalmente

ERR-40: descripción con backticks
  Input:  lql create "Fix bug" -d 'Usar `json.dumps()` para escapar'
  Expect: issue creada, backticks preservados

ERR-41: descripción con newlines (vía --description-file)
  Input:  echo -e "## Problema\n\nEl token expira.\n\n## Fix\n\nDetectar expiración." > /tmp/desc.md && lql create "Fix bug" --description-file /tmp/desc.md
  Expect: issue creada, markdown preservado con newlines

ERR-42: descripción con $variables (no expandidas)
  Input:  lql create "Fix bug" -d 'Set $PATH to include ~/.local/bin'
  Expect: "$PATH" literal en la descripción, no expandido

ERR-43: descripción con backslashes
  Input:  lql create "Fix bug" -d 'Regex: \\d+\\.\\d+'
  Expect: backslashes preservados

ERR-44: descripción con emojis y unicode
  Input:  lql create "Fix bug" -d '⚠️ Error en producción — 日本語テスト'
  Expect: unicode preservado

ERR-45: stdin con heredoc
  Input:  lql create "Fix bug" <<'EOF'
          ## Problema
          El campo "title" tiene `backticks` y $variables.
          EOF
  Expect: issue creada, todo preservado literalmente
```

### Auth — 1Password failures

```
ERR-46: op read timeout
  Input:  (simular op read que devuelve exit 1 con "authorization timeout")
  Expect: exit 1, stderr: "✗ Could not read API key from 1Password.\n  Run: op read \"op://<your-vault>/Linear/api-key\"\n  If this fails, check: op signin"

ERR-47: op read dismissed
  Input:  (simular op read que devuelve exit 1 con "authorization prompt dismissed")
  Expect: mismo mensaje que ERR-46
```

### API — Error handling

```
ERR-48: Linear API devuelve error GraphQL
  Input:  (simular respuesta {"errors": [{"message": "Entity not found"}]})
  Expect: exit 1, stderr: "✗ Linear API error: Entity not found"

ERR-49: Linear API devuelve 429 (rate limit)
  Input:  (simular HTTP 429)
  Expect: retry con backoff (2s, 4s, 8s), max 3 retries, luego error claro

ERR-50: Linear API devuelve 401
  Input:  (simular HTTP 401)
  Expect: exit 1, stderr: "✗ Authentication failed. Check your API key: lql doctor"

ERR-51: Linear API devuelve 500
  Input:  (simular HTTP 500)
  Expect: retry con backoff, luego: "✗ Linear API server error (500). Try again later."

ERR-52: Network error (no conexión)
  Input:  (simular connection refused)
  Expect: exit 1, stderr: "✗ Could not connect to Linear API. Check your network."

ERR-53: issue no encontrada
  Input:  lql view PROD-99999
  Expect: exit 1, stderr: "✗ PROD-99999 not found."

ERR-54: issue no encontrada con sugerencia
  Input:  lql view PROD-999
  Expect: exit 1, stderr: "✗ PROD-999 not found. Similar: PROD-599 \"OAuth token refresh\""
```

### Formato de output

```
ERR-55: list output es compacto (una línea por issue)
  Input:  lql list --team PROD --limit 3
  Expect: cada línea sigue formato "ID [State] labels — Title (age, due)"

ERR-56: list footer muestra conteo por estado
  Input:  lql list --team PROD
  Expect: última línea: "── N issues (X backlog, Y todo, Z in-progress)"

ERR-57: create output muestra ID + URL
  Input:  lql create "Test issue" --team PROD --label tokamak
  Expect: "✓ PROD-XXX created [Todo] tokamak — Test issue\n  https://linear.app/frr149/issue/PROD-XXX"

ERR-58: update output muestra transición
  Input:  lql update PROD-587 --state completed
  Expect: "✓ PROD-587 Backlog → Done"

ERR-59: --json produce JSONL válido
  Input:  lql list --json --limit 3
  Expect: cada línea es JSON válido parseable con serde, campos: id, state, labels, title, age_days, due, overdue, project, priority

ERR-60: output no contiene ANSI escape codes
  Input:  lql list | cat -v
  Expect: no hay secuencias \e[, \033[, etc.
```

### Búsqueda

```
ERR-61: search encuentra por título
  Input:  lql search "basedpyright"
  Expect: devuelve issues con "basedpyright" en título o descripción, formato list

ERR-62: search con filtro de team
  Input:  lql search "OAuth" --team PROD
  Expect: solo issues del team PROD

ERR-63: search con filtro de estado
  Input:  lql search "OAuth" --state backlog,unstarted
  Expect: solo issues en esos estados

ERR-64: search sin resultados
  Input:  lql search "xyznonexistent123"
  Expect: "── 0 issues", exit 0
```

### Comentarios

```
ERR-65: comment inline
  Input:  lql comment PROD-587 "Investigado, el problema es X"
  Expect: "✓ Comment added to PROD-587"

ERR-66: comment desde fichero
  Input:  echo "## Progreso" > /tmp/c.md && lql comment PROD-587 --file /tmp/c.md
  Expect: "✓ Comment added to PROD-587"

ERR-67: comment desde stdin
  Input:  echo "Progreso parcial" | lql comment PROD-587
  Expect: "✓ Comment added to PROD-587"
```

### Relaciones

```
ERR-68: relate blocks
  Input:  lql relate PROD-587 blocks PROD-588
  Expect: "✓ PROD-587 blocks PROD-588"

ERR-69: relate blocked-by se normaliza
  Input:  lql relate PROD-587 blocked-by PROD-515
  Expect: crea relación PROD-515 blocks PROD-587 (invertida), "✓ PROD-587 blocked-by PROD-515"

ERR-70: relate related
  Input:  lql relate PROD-587 related PROD-520
  Expect: "✓ PROD-587 related PROD-520"

ERR-71: tipo de relación inválido
  Input:  lql relate PROD-587 depends-on PROD-520
  Expect: exit 1, stderr: "✗ Unknown relation type \"depends-on\". Available: blocks, blocked-by, related"
```

### Duplicados

```
ERR-72: create detecta duplicado y avisa
  Input:  lql create "OAuth token refresh" --team PROD --label tokamak
  Expect: stderr warning con issues similares, pero crea de todos modos (no-TTY)

ERR-73: create con --force omite detección de duplicados
  Input:  lql create "OAuth token refresh" --team PROD --label tokamak --force
  Expect: crea sin warning
```

### Concurrencia

```
ERR-74: dos lql list simultáneos no interfieren
  Input:  lql list --team PROD & lql list --team CONT & wait
  Expect: ambos completan sin error

ERR-75: lql create + lql list simultáneos
  Input:  lql create "Test" --team PROD --label tokamak & lql list --team PROD & wait
  Expect: ambos completan sin error
```
