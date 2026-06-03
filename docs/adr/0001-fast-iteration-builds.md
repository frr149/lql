# 0001 — Fast iteration builds (test compile times)

- Status: Accepted
- Date: 2026-06-03

## Context

`cargo test` was slow even when only production code changed, not the tests.
The reasons are inherent to how Rust and macOS build test code:

1. **The crate is the unit of compilation, not the file.** Unit tests live
   inside the crate (`#[cfg(test)] mod tests`) and link against all of its
   production code. Changing any source file recompiles the whole crate in its
   `--cfg test` variant. There is no "recompile only the tests" for unit tests.
   (Integration tests under `tests/` are separate crates, so each is its own
   binary; changing one only rebuilds that crate, but changing the library
   rebuilds the library _and_ every integration crate.)

2. **Tests are separate binaries — several of them.** `cargo test` builds: a
   test binary for the library, a test binary for the bin, one binary per file
   in `tests/`, and the doctests. Each links the full dependency graph
   (clap, reqwest, serde, …).

3. **This crate paid for it twice.** `lib.rs` exposed 6 modules
   (`auth, cli, client, config, format, queries`) while `main.rs` declared 8
   with its own `mod` (`+ commands, middleware`). The 6 shared modules — and
   their tests — were compiled into _both_ the lib test binary and the bin test
   binary, and the overlapping tests ran twice. Observed before this change:
   156 tests in the lib binary **plus** 245 in the bin binary, heavily
   overlapping.

4. **macOS makes linking the bottleneck.** After compilation, the linker stitches
   objects into each executable. On macOS this is aggravated by: the system
   linker (`ld64`), ad-hoc **code-signing of every binary** (mandatory on Apple
   Silicon), and **debug-info / dSYM** handling (`dsymutil`). Multiply by the
   number of test binaries and the tail of every `cargo test` is link + sign,
   not type-checking.

## Decision

### Committed to the repo (universal, no external dependency)

1. **Thin binary.** All modules live in the **library** crate. `main.rs` is a
   shell: `fn main() { std::process::exit(lql::run()) }`. The CLI entry point
   moved to `lql::run()` in `lib.rs`. Result: unit tests compile and run **once**
   (lib test binary); the bin test binary is empty. This removes the duplicate
   compilation/execution and is cleaner architecture (the binary is a thin
   adapter over the library). NOTE: this was expected to roughly halve build
   time; measurement (see Consequences) showed the wall-clock effect is
   negligible — keep this change for the architecture, not the speed.

2. **`debug = "line-tables-only"`** for the `dev` and `test` profiles in
   `Cargo.toml`. Keeps `file:line` in panics/backtraces while cutting the symbol
   generation, linking and code-signing cost. Full debuginfo (`debug = true`)
   is not needed for the normal test loop.

### Local-only, not committed (documented here, set up per machine)

These require external tools and would force that dependency onto CI and any
contributor if committed, so they live in the **user-global** `~/.cargo/config.toml`
and the developer's shell — never in the repo:

- **sccache** — a compilation cache (think `ccache` for Rust, from Mozilla). It
  wraps `rustc` (`RUSTC_WRAPPER=sccache`) and returns a cached artifact when the
  same crate is compiled again — the big win when switching between feature
  branches and back. Trade-off: it is incompatible with incremental compilation
  (`CARGO_INCREMENTAL=0`), so it swaps fine-grained incremental rebuilds for
  cross-invocation caching. Net positive for a branch-heavy / agent-driven
  workflow.

      # ~/.cargo/config.toml  (machine-global, not in this repo)
      [build]
      rustc-wrapper = "sccache"

- **A faster linker.** `lld` (LLVM's Mach-O linker `ld64.lld`, via
  `brew install lld`) or `mold` (faster still, but Linux-only — its macOS port
  was discontinued). **Measured on this repo and found NOT worth it — see
  Consequences.** Kept here only as a documented option for a future,
  link-heavier state. Configure per target so CI (Linux) and other contributors
  are unaffected:

      # ~/.cargo/config.toml
      [target.aarch64-apple-darwin]
      rustflags = ["-C", "link-arg=-fuse-ld=lld"]

### Workflow (no config)

- Iterate with a **subset**: `cargo test --lib <filter>` compiles and runs only
  the library test binary, skipping integration crates and doctests. Reserve the
  full `cargo test` for pre-commit verification.
- Do **not** chain `cargo clippy` and `cargo test` during iteration: they are
  separate compilations (clippy is check-only, test does codegen), so running
  both doubles the wait. Run clippy once before committing.
- Optional: **`cargo nextest`** runs the suite with less per-test overhead and
  better output. It speeds the _run_ phase, not compilation (which is the
  bottleneck here), so it is a nice-to-have, not the main lever.

## Consequences

### Measured (2026-06-03, Apple Silicon, sccache off unless noted)

| Scenario                                        | Old (v1.7.0) | New (v1.7.1) |
| ----------------------------------------------- | -----------: | -----------: |
| Incremental rebuild + test (touch one src file) | 2.86 s       | 2.99 s       |
| Cold `cargo test` (clean target)                | 16.66 s      | 16.30 s      |
| Cold `cargo test`, sccache cache warm           | —            | 7.32 s       |

**Honest result, against the original expectation:**

- The thin-binary refactor and `line-tables-only` change have a **negligible
  wall-clock effect** (~2% cold, none incremental). Cold time is dominated by
  compiling the dependency graph (clap, reqwest, serde, …), identical either
  way; incremental rebuilds are dominated by fixed link/cargo overhead. The
  earlier "halves build time" framing conflated the _test-count duplication_
  (243 in one binary vs 156 + 245 across two — real) with _wall-clock build
  time_ (which barely moves). Claiming a speedup without measuring it was the
  mistake.
- The thin-binary refactor is kept for **architecture, not speed**: the library
  is the product, the binary a thin adapter, and the unit tests run once.
- **sccache is the only real lever**: a clean build with a warm cache is ~2.2×
  faster (16.3 s → 7.3 s) — exactly the branch-switch / agent-rebuild case.
- **A faster linker (lld) does not help here either** — measured, not assumed
  (`cargo test --no-run`, sccache off): cold 13.9 s with Apple `ld` vs 13.4 s
  with Homebrew `lld` 22.1.6 (~3%, within noise); incremental relink identical
  (~1.03 s both). The build is compile-bound (dependency compilation dominates,
  which the linker never touches) and Apple's current linker is already fast on
  Apple Silicon, so lld has nothing to bite on. The lld-linked binary was
  verified to run correctly. `mold` is not an option on macOS at all — it is
  ELF-only (its macOS port `sold` is discontinued). Verdict: **keep the system
  linker**; lld/mold are not worth the dependency for this repo.
- Backtraces keep `file:line` (line-tables-only) but lose full variable debug
  info — acceptable for the test loop; `dist`/`release` profiles are unchanged.
- The aggressive tooling (sccache, lld) stays opt-in and machine-local, so CI
  and contributors are never forced to install anything new.
