# MDD Layer 2 Findings — Adversarial Testing with lql

## Methodology

Use lql for real tasks. Document every error, surprise, difficulty, and success.
Each finding becomes either:
- A new normalization/tolerance to add to lql (adapt the sane to the insane)
- A test case (verify the adaptation works)
- Evidence that Layer 1 design was correct (or had gaps)

## Session: 2026-03-26 (first use after TOON integration)

### Finding 1: cargo warnings pollute output (NOISE)

**What happened:** Running `cargo run -- list` dumps 20+ lines of compiler warnings before the actual output. The LLM has to filter them out mentally.

**Category:** Noise, not an lql bug — but affects usability.

**Fix:** Build release once (`cargo build --release`), use `./target/release/lql`. Or suppress warnings with `RUSTFLAGS="-A warnings"` during dev. Not an lql issue — Sancho stack doesn't need to handle this.

**Layer 1 validation:** N/A (build system, not CLI).

### Finding 2: --status alias works silently (SUCCESS)

**What happened:** Used `--status backlog` instead of `--state backlog`. It worked. No stderr message about normalization (unlike `--state Todo` which prints `ℹ State "Todo" → normalized to "unstarted"`).

**Why:** `--status` is a clap alias for `--state`. Since the VALUE `backlog` is already a valid API value, no normalization happens. The flag name normalization is silent by design (clap handles it).

**Question from user:** Should `--status` emit a warning? See Finding 7 below.

**Layer 1 validation:** ✅ Flag alias works as designed (ERR-02).

### Finding 3: State normalization messages are helpful (SUCCESS)

**What happened:** `--state Done` prints `ℹ State "Done" → normalized to "completed"` to stderr. The LLM learns the correct value for next time.

**Layer 1 validation:** ✅ Exactly as designed in PRD.

### Finding 4: Priority normalization works (SUCCESS)

**What happened:** `--priority high` prints `ℹ Priority "high" → normalized to 2` and applies correctly.

**Layer 1 validation:** ✅ (ERR-08).

### Finding 5: Label validation catches invented labels (SUCCESS)

**What happened:** `--label kubernetes` rejected with full list of available labels.

**Observation:** The error message lists ALL 48 labels. That's a lot of tokens. Should truncate to ~10 most similar + count of total.

**Action:** Truncate available labels list in error messages to save tokens.

**Layer 1 validation:** ✅ (ERR-23), but error message is too verbose.

### Finding 6: TOON output is clean and readable (SUCCESS)

**What happened:** List output is tabular TOON with header row. Easy to scan, fields are self-describing.

**Observation:** The `project` field is often empty (`""`). Could omit empty fields or use null. Also `due` is often empty — same issue.

**Potential improvement:** In TOON, could we omit fields that are empty for ALL issues in the batch? Or use a more compact representation for empty values?

**Layer 1 validation:** TOON format works as expected.

### Finding 7: Flag normalization should be visible but not block (DESIGN QUESTION)

**User feedback:** `--status` should work (it does) but should ALSO emit a warning like state normalization does: `ℹ --status → normalized to --state`. This teaches the LLM the correct flag name without breaking the command.

**Current behavior:** Silent (clap alias, no message).

**Proposed behavior:** clap alias still works, but lql adds stderr message.

**Problem:** clap aliases are transparent — by the time our code runs, `--status` is already `--state`. We can't detect which flag the user typed.

**Possible solutions:**
1. Don't use clap aliases — do raw arg parsing and normalize manually (more code, more control)
2. Accept current behavior (silent alias is fine, the LLM doesn't need to learn)
3. Use clap's `value_parser` with a wrapper that logs

**Recommended:** Option 2 for now. The goal of `--status` alias is to NOT fail, not to educate. Education happens with state VALUES (Todo→unstarted) which are visible. Flag education is less critical because the flag WORKS either way.

### Finding 8: TOON integration with footer works (SUCCESS)

**What happened:** TOON body + compact footer (`── N issues (X backlog, Y todo)`) combine cleanly. The footer is NOT in TOON format — it's a separate line. This works because the footer is metadata about the result, not a data row.

**Layer 1 validation:** ✅ Footer design is correct.

### Finding 9: create output uses old compact format (INCONSISTENCY)

**What happened:** `lql create` still outputs `✓ TOOL-51 created [Backlog] lql — Title` (compact format, not TOON). Same for `update` (`✓ TOOL-51 In Progress → Done`) and `comment` (`✓ Comment added`).

**Is this a problem?** No — single-item confirmations don't benefit from TOON tabular format. TOON is for arrays of uniform objects. A single confirmation is better as a one-liner.

**Decision:** Keep compact format for create/update/comment/relate confirmations. TOON only for list/search (array outputs).

**Layer 1 validation:** ✅ Correct separation of formats by use case.

### Finding 10: view still uses custom format (EXPECTED)

**What happened:** `lql view TOOL-51` outputs the multi-line detail view (not TOON). This is correct — view shows ONE issue with description, relations, comments. Not tabular.

**Layer 1 validation:** ✅ View is deliberately different from list.

## Summary

| Category | Count | Details |
|----------|-------|---------|
| ✅ Layer 1 correct | 8 | Aliases, normalization, validation, formats |
| ⚠ Improvement needed | 1 | Label error message too verbose (#5) |
| 💡 Design question | 1 | Flag name normalization visibility (#7) |
| 📝 Noise | 1 | Cargo warnings (#1) |

**Layer 1 hit rate: 8/10 findings validated the original design.** One improvement needed (verbose error), one open design question (flag visibility). No critical gaps found.

**Key MDD insight:** The biggest surprise was NOT an error — it was finding that everything worked as designed. The 500+ historical errors from Layer 1 successfully predicted what Layer 2 would test. The two-layer approach validates itself: Layer 1 built the right defenses, Layer 2 confirmed they hold.

## Core Principle (emerged from discussion)

**Adapt what doesn't destroy. Reject what destroys. Always say what you assumed.**

This is Postel's Law (TCP robustness principle: "be conservative in what you send, liberal in what you accept") with two additions:

1. **Liberality limit:** if accepting would corrupt data, reject
2. **Transparency:** always state what was received vs what was assumed

| Input | Destructive? | Action | Message |
|-------|-------------|--------|---------|
| `--status Done` | No | Accept | `ℹ --status → assumed --state` |
| `--state Todo` | No | Normalize | `ℹ State "Todo" → normalized to "unstarted"` |
| `--priority urgent` | No | Normalize | `ℹ Priority "urgent" → normalized to 1` |
| `--label kubernetes` | **Yes** (garbage in Linear) | **Reject** | `✗ Label "kubernetes" not found` |
| `--team TOK` | **Yes** (retired team) | **Reject** | `✗ Team TOK is retired` |

This principle is the Sancho Panza contract: redirect the charge when it hits a windmill, but tackle Don Quixote to the ground when he's about to ride off a cliff.
