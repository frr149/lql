# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
