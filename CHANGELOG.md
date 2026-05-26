# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.6.0](https://github.com/frr149/lql/releases/tag/v1.6.0) - 2026-05-26

### Added

- `lql epic update` — update title, body (`--description` / `--description-file`), summary, and target date on an existing epic. Applies the change to both the Linear initiative and its single backing project atomically. Fails loud with a hint when the epic has zero or more than one backing projects.
- `lql epic comment` — add a comment to an existing epic. Writes to the initiative directly and mirrors the comment onto the backing project when one exists, so it shows up in both surfaces.
- `lql project view / update / comment` — generic project commands accepting UUID, slugId, or case-insensitive name as the project reference. Closes the agent escape hatch that required `lql raw 'mutation { projectUpdate(...) }'` for normal planning workflows.
- `lql epic view` default (non-JSON) output now exposes the backing project's UUID, slugId, and URL, so a follow-up command does not need a second introspection round.

## [1.5.1](https://github.com/frr149/lql/releases/tag/v1.5.1) - 2026-05-20

### Fixed

- repair fully-broken `epic` subcommand — complexity-limit queries, long-body overflow into the capped `description` field, non-atomic `create` leaving orphans, UUID-filter validation, and the 80-char backing-project name limit
- fetch comment body/user in GraphQL query, add `comments` subcommand
- sanitize personal identifiers across all tests and fixtures

## [1.5.0](https://github.com/frr149/lql/releases/tag/v1.5.0) - 2026-05-06

### Added

- show key dates (startedAt, completedAt, updatedAt) in view output

## [1.4.1](https://github.com/frr149/lql/releases/tag/v1.4.1) - 2026-04-29

### Added

- relate defaults to "related", normalize "relates-to"

## [1.4.0](https://github.com/frr149/lql/releases/tag/v1.4.0) - 2026-04-29

### Added

- agentic experience — tolerate common LLM mistakes
- support LINEAR_API_KEY env var to avoid repeated 1Password prompts
- add unlink command and remove joke banner
- add label create and delete subcommands
- add --no-label filter to list command
- add Telegram notification when release PR is created
- add release-plz for automated versioning and changelog
- add cargo-dist for cross-platform binary releases
- add arg middleware, truncate label errors, complete 64/75 ERR tests
- adopt TOON as default output format, replacing custom compact
- implement lql MVP — all commands, 31 tests passing

### Fixed

- correct stale integration test assertions
- correct author email
- update labels help text to reflect subcommands
- configure HOMEBREW_TAP_TOKEN for formula publishing
- drop aarch64-pc-windows-msvc target (unsupported in CI)
- switch to rustls-tls for cross-platform builds
- correct Linear API filter syntax for states and sort

### Other

- bump version to 1.4.0
- bump version to 1.3.0
- Merge remote-tracking branch 'origin/codex/epic-support'
- publish Homebrew formula to shared tap
- bump version to 1.2.1
- bump version to 1.2.0
- release v1.1.0 ([#1](https://github.com/frr149/lql/pull/1))
- tweak CLI banner to mention Rust
- Harden lql machine-mode output
- extract curator scope from lql PRD
- add install instructions for all platforms
- bump version to 1.0.0
- add badges to README
- add comprehensive README with adversarial programming methodology
- translate all user-facing text to English
- add MIT license, repository metadata, install recipe
- clean up dead code, zero warnings
- add justfile for common tasks
- extract GraphQLClient trait + CommandRunner for testability
- add Layer 1 hit rate KPI to MDD findings
- add Sancho Panza Contract — core MDD principle
- add MDD Layer 2 findings from first adversarial session
- add 98 tests — validation, escapado, PBT, identifiers
- add fixture-based tests with real Linear API responses
- add 75 test cases derived from real error audit
- add edge cases from error audit, rename to lql
- PRD and CLAUDE.md for lql (Linear Query Language)

## [1.1.0](https://github.com/frr149/lql/releases/tag/v1.1.0) - 2026-03-31

### Added

- add unlink command and remove joke banner
- add label create and delete subcommands
- add --no-label filter to list command
- add Telegram notification when release PR is created
- add release-plz for automated versioning and changelog
- add cargo-dist for cross-platform binary releases
- add arg middleware, truncate label errors, complete 64/75 ERR tests
- adopt TOON as default output format, replacing custom compact
- implement lql MVP — all commands, 31 tests passing

### Fixed

- update labels help text to reflect subcommands
- configure HOMEBREW_TAP_TOKEN for formula publishing
- drop aarch64-pc-windows-msvc target (unsupported in CI)
- switch to rustls-tls for cross-platform builds
- correct Linear API filter syntax for states and sort

### Other

- tweak CLI banner to mention Rust
- Harden lql machine-mode output
- extract curator scope from lql PRD
- add install instructions for all platforms
- bump version to 1.0.0
- add badges to README
- add comprehensive README with adversarial programming methodology
- translate all user-facing text to English
- add MIT license, repository metadata, install recipe
- clean up dead code, zero warnings
- add justfile for common tasks
- extract GraphQLClient trait + CommandRunner for testability
- add Layer 1 hit rate KPI to MDD findings
- add Sancho Panza Contract — core MDD principle
- add MDD Layer 2 findings from first adversarial session
- add 98 tests — validation, escapado, PBT, identifiers
- add fixture-based tests with real Linear API responses
- add 75 test cases derived from real error audit
- add edge cases from error audit, rename to lql
- PRD and CLAUDE.md for lql (Linear Query Language)
