# SAK Phase 1 & 2 Summary — Completion Report

**Date:** May 5, 2026  
**Status:** ✅ Both phases complete and verified

---

## Phase 1 — sak-core (Shared Foundation)

**Crate:** `crates/sak-core`

### Types Implemented

| Type | Purpose | Location |
|------|---------|----------|
| `Decision` | Guardian verdict: `Allow` or `Reject { rule, reason }` | `types.rs` |
| `TxMeta` | Agent intent metadata (slippage_bps, description) | `types.rs` |
| `ChainEvent` / `EventKind` | Slot-stamped events from Reflex Engine | `types.rs` |
| `SakError` | Top-level error enum using `thiserror` | `error.rs` |

### Build Verification

```bash
cargo build -p sak-core
```

✅ Compiles with zero warnings  
✅ All types implement `Debug`, `Clone`, `Serialize`, `Deserialize` where appropriate

---

## Phase 2 — sak-guardian (Pillar 2: Safety Layer)

**Crate:** `crates/sak-guardian`

### Architecture

```
sak-guardian/
├── src/
│   ├── lib.rs          # Guardian struct + public API
│   ├── rules.rs        # Rule types + YAML deserialization
│   └── evaluator.rs    # Rule evaluation logic + instruction parsers
└── tests/
    └── evil_corpus.rs  # 20 malicious transaction patterns
```

### Rules Implemented (7 total)

| Rule | YAML Type | What It Blocks |
|------|-----------|----------------|
| `max_slippage` | `slippage_check` | Slippage above configured basis point cap |
| `allowed_programs` | `program_whitelist` | Instructions invoking non-whitelisted programs |
| `max_account_drain` | `drain_check` | System transfers above lamport drain limit |
| `max_accounts` | `account_count_check` | Transactions with too many accounts (obfuscation) |
| `max_compute_units` | `compute_units_check` | Excessive compute unit requests |
| `max_priority_fee` | `priority_fee_check` | Excessive priority fees |
| `min_transfer_value` | `min_transfer_lamports` | Zero-amount dust attacks |

**Stubs (always pass):** `value_check`, `decimals_check`

### Guardian Public API

```rust
// Load from YAML
let guardian = Guardian::from_yaml("rules.yaml")?;

// Or construct directly (useful for tests)
let guardian = Guardian::with_rules(vec![Rule::SlippageCheck { name, max_bps }]);

// Evaluate a transaction
let decision = guardian.evaluate_raw(account_keys, &instructions, &meta);

match decision {
    Decision::Allow => { /* proceed to sign */ }
    Decision::Reject { rule, reason } => {
        // zero on-chain cost — tx never left the machine
    }
}
```

### Evil Corpus Test Results

**File:** `crates/sak-guardian/tests/evil_corpus.rs`  
**Command:** `cargo test -p sak-guardian`

```
running 20 tests
test blocks_99_percent_slippage          ... ok
test blocks_wrong_token_mint             ... ok
test blocks_full_wallet_drain             ... ok
test blocks_unknown_program               ... ok
test blocks_disguised_fee_drain           ... ok
test blocks_max_u64_slippage              ... ok
test blocks_unlisted_jupiter_pool         ... ok
test blocks_zero_amount_dust_attack       ... ok
test blocks_account_substitution_drain    ... ok
test blocks_balance_underflow             ... ok
test blocks_excessive_compute_units      ... ok
test blocks_reentrancy_cpi_loop          ... ok
test blocks_fake_system_program           ... ok
test blocks_multiple_drain_instructions   ... ok
test blocks_slippage_bypass_via_cpi       ... ok
test blocks_token_account_closed_mid_tx   ... ok
test blocks_excessive_priority_fee        ... ok
test blocks_memo_injection                ... ok
test blocks_excessive_account_count      ... ok
test blocks_unverified_program_mainnet_fail ... ok

test result: ok. 20 passed; 0 failed; 0 ignored
```

✅ **All 20/20 evil corpus tests pass**

### Rules Configuration

**File:** `rules.yaml` (workspace root)

```yaml
rules:
  - name: max_slippage
    type: slippage_check
    max_bps: 200

  - name: allowed_programs
    type: program_whitelist
    programs:
      - JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4   # Jupiter v6
      - whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc    # Orca Whirlpool
      - 11111111111111111111111111111111                  # System program
      - TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA    # SPL Token
      - ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bJ     # Associated Token Account
      - ComputeBudget111111111111111111111111111111       # Compute Budget

  - name: max_account_drain
    type: drain_check
    max_lamports: 1000000000

  - name: max_accounts
    type: account_count_check
    max_count: 20

  - name: max_compute_units
    type: compute_units_check
    max_units: 1400000

  - name: max_priority_fee
    type: priority_fee_check
    max_microlamports: 1000000

  - name: min_transfer_value
    type: min_transfer_lamports
    min_lamports: 1
```

---

## Code Quality Verification

| Check | Command | Result |
|-------|---------|--------|
| Build | `cargo build -p sak-core && cargo build -p sak-guardian` | ✅ Pass |
| Tests | `cargo test -p sak-guardian` | ✅ 20/20 pass |
| Clippy | `cargo clippy -p sak-core -p sak-guardian -- -D warnings` | ✅ No warnings |
| Format | `cargo fmt --check` | ✅ Formatted |

---

## Key Implementation Details

### Instruction Parsers (evaluator.rs)

- **System program:** Parses bincode-encoded instructions (discriminant 2 = transfer, 0 = create_account) to extract lamport amounts
- **ComputeBudget program:** Parses 1-byte discriminant instructions (0x02 = SetComputeUnitLimit, 0x03 = SetComputeUnitPrice)

### Design Decisions

1. **No LiteSVM simulation yet** — The current implementation evaluates rules against transaction metadata and instruction data without running LiteSVM simulation. This is a deliberate simplification for the hackathon demo.
2. **TxView pattern** — Decouples evaluator from specific Solana SDK versions by using a minimal view struct
3. **Rule stubs** — `ValueCheck` and `DecimalsCheck` are defined but always pass (marked with TODO)

---

## Next Steps (Phase 3)

**Goal:** Demo UI — Live safety log visible to hackathon judges

**Architecture:**
```
sak-guardian (Rust) → tracing events
        ↓
axum WebSocket server → streams JSON
        ↓
React UI — live rejection log table
```

**What it shows:**
- Every transaction evaluated in real time
- `[BLOCKED]` entries in red with rule name and reason
- `[ALLOWED]` entries in green
- Counter of total blocked vs allowed

**Target completion:** May 6, 2026

---

## Summary

Phase 1 (sak-core) and Phase 2 (sak-guardian) are **complete and verified**. The Guardian rule engine blocks all 20 known malicious LLM transaction patterns. The codebase is clean, tested, and ready for the Demo UI phase.

**Lines of code:**
- `sak-core`: ~50 lines (types + errors)
- `sak-guardian`: ~300 lines (rules + evaluator + API)
- `evil_corpus.rs`: ~414 lines (20 tests)

**Hackathon deadline:** May 11, 2026 — **6 days remaining**
