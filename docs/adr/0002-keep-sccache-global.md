# 0002 — Keep sccache enabled machine-global (the agent is the primary compiler)

- Status: Accepted
- Date: 2026-06-03
- Relates to: [0001](0001-fast-iteration-builds.md) (measured the tradeoff)

## Context

ADR 0001 measured sccache on this repo:

- Clean build / branch switch with a warm cache: **~2.2× faster** (16.3 s → 7.3 s).
- But sccache forces `CARGO_INCREMENTAL=0`, so the **single-file edit loop is ~0.5 s
  slower** (1.03 s → 1.51 s on this crate).

0001 left adoption open (documented it as opt-in). The choice hinges entirely on
workload mix: clean builds and branch switches favour sccache; tight single-file
edit loops favour incremental compilation. They are mutually exclusive (an
incrementally-compiled crate is not content-addressable, so sccache can't cache it).

The deciding fact for **this** setup: the primary compiler is the **AI agent**
(Claude Code), not a human in a tight TDD loop. The agent's observed pattern is
branch-heavy — it creates feature branches, switches between them, uses git
worktrees, and runs clean/full builds far more than it does single-file edit loops.
And a clean build can't use incremental anyway, so sccache is **pure upside** there;
fresh worktrees (each with a cold `target/`) hit the shared cache.

## Decision

Keep sccache enabled machine-global in `~/.cargo/config.toml`:

    [build]
    rustc-wrapper = "sccache"
    incremental = false

## Consequences

- Clean builds / branch switches / fresh worktrees: ~2.2× faster — the agent's
  dominant operations.
- Single-file edit loop: ~0.5 s slower (incremental off). Small on a crate this size;
  could be larger on a big Rust crate during a long **hand**-edit session.
- Escape hatch (no permanent change): run one session without sccache —
  `RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo test` — or comment out `rustc-wrapper`.
- **Scope — Rust (and C/C++/CUDA), not Swift.** sccache supports gcc, clang, MSVC,
  rustc, NVCC/NVC++, hipcc, diab. It does **not** support `swiftc`, so it does
  nothing for Swift projects (e.g. Tokamak — whose build lever is the type-checker,
  see Tokamak `docs/adr/007`, and whose cache equivalent is Apple's emerging
  compilation caching, not sccache). For this stack the practical benefit is
  Rust-mostly.
- Local cache: `~/Library/Caches/Mozilla.sccache`, 10 GiB LRU cap (self-managing).
