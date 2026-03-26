# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.3](https://github.com/frr149/lql/releases/tag/v1.0.3) - 2026-03-26

### Added

- add Telegram notification when release PR is created
- add release-plz for automated versioning and changelog
- add cargo-dist for cross-platform binary releases
- add arg middleware, truncate label errors, complete 64/75 ERR tests
- adopt TOON as default output format, replacing custom compact
- implement lql MVP — all commands, 31 tests passing

### Fixed

- configure HOMEBREW_TAP_TOKEN for formula publishing
- drop aarch64-pc-windows-msvc target (unsupported in CI)
- switch to rustls-tls for cross-platform builds
- correct Linear API filter syntax for states and sort

### Other

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
