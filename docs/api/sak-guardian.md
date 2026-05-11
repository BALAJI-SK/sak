# sak-guardian — Rule Evaluation API

Pre-sign transaction simulation + configurable rule engine. Runs every transaction through LiteSVM and checks against a YAML-defined rule set before the agent ever signs.

```
Cargo.toml: sak-guardian = { path = "../crates/sak-guardian" }
```

## Guardian

```rust
pub struct Guardian {
    // private: rules: RuleSet, simulator: Simulator
}
```

### Guardian::from_yaml

```rust
pub fn from_yaml(path: impl AsRef<Path>) -> Result<Self>
```

Load rules from a YAML file. Each rule is deserialized from a `#[serde(tag = "type")]` enum — see the [Rule YAML Schema](#rule-yaml-schema) below.

### Guardian::from_yaml_with_svm

```rust
pub fn from_yaml_with_svm(
    path: impl AsRef<Path>,
    svm: litesvm::LiteSVM,
) -> Result<Self>
```

Load rules from YAML but use an externally-provided `LiteSVM` instance (useful when you need pre-seeded accounts).

### Guardian::with_rules

```rust
pub fn with_rules(rules: Vec<Rule>) -> Self
```

Construct from a `Vec<Rule>` directly. Useful for tests where you don't want a YAML file on disk.

### Guardian::evaluate

```rust
pub fn evaluate(
    &mut self,
    tx: &VersionedTransaction,
    meta: &TxMeta,
) -> Decision
```

**Full simulation path.** Runs the transaction through LiteSVM, snapshots pre/post balances, then evaluates all rules. Returns `Decision::Allow` or `Decision::Reject { rule, reason }`.

```rust
let mut guardian = Guardian::from_yaml("rules.yaml")?;
let decision = guardian.evaluate(&versioned_tx, &TxMeta {
    slippage_bps: Some(50),
    description: None,
});
```

### Guardian::evaluate_raw

```rust
pub fn evaluate_raw(
    &self,
    account_keys: Vec<String>,
    instructions: &[(u8, &[u8])],
    meta: &TxMeta,
) -> Decision
```

**No-simulation path.** Evaluates rules using only account keys and instruction data — no LiteSVM needed. Each instruction tuple is `(program_id_index, instruction_data)`, mirroring a compiled Solana message.

```rust
let decision = guardian.evaluate_raw(
    vec![payer.to_string(), JUP6_PROGRAM.to_string()],
    &[(1, &[])],  // call Jupiter with no data
    &TxMeta { slippage_bps: Some(9900), description: None },
);
```

## Rule YAML Schema

Rules are defined in a YAML file and loaded via `from_yaml`. Each rule has a `type` tag and a `name`. Rules evaluate in order — first rejection short-circuits.

```yaml
rules:
  - name: max_slippage
    type: slippage_check
    max_bps: 200

  - name: allowed_programs
    type: program_whitelist
    programs:
      - "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"
      - "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
      - "11111111111111111111111111111111"

  - name: max_account_drain
    type: drain_check
    max_lamports: 1000000000

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

### Rule Variants

| `type` | Struct | Key Field | Evaluates |
|---|---|---|---|
| `slippage_check` | `SlippageCheck` | `max_bps: u64` | Agent-declared slippage (from `TxMeta`) |
| `program_whitelist` | `ProgramWhitelist` | `programs: Vec<String>` | All program IDs in the message |
| `drain_check` | `DrainCheck` | `max_lamports: u64` | Transfers to System Program |
| `account_count_check` | `AccountCountCheck` | `max_count: usize` | Total accounts in message |
| `compute_units_check` | `ComputeUnitsCheck` | `max_units: u32` | ComputeBudget SetComputeUnitLimit |
| `priority_fee_check` | `PriorityFeeCheck` | `max_microlamports: u64` | ComputeBudget SetComputeUnitPrice |
| `min_transfer_lamports` | `MinTransferLamports` | `min_lamports: u64` | Minimum transfer amount (dust prevention) |
| `value_check` | `ValueCheck` | `max_usd: f64` | **Stub** — always passes |
| `decimals_check` | `DecimalsCheck` | `token: String`, `expected_decimals: u8` | **Stub** — always passes |

All variants include a `name: String` field.

## RuleSet

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}
```

## TxView

Internal enum that decouples the evaluator from Solana SDK types:

```rust
pub enum TxView<'a> {
    Raw {
        account_keys: Vec<String>,
        instructions: &'a [(u8, &'a [u8])],
    },
    Simulated {
        account_keys: Vec<String>,
        instructions: Vec<(u8, Vec<u8>)>,
        pre_balances: HashMap<String, u64>,
        post_balances: HashMap<String, u64>,
    },
}
```

Created automatically by `evaluate` (from `VersionedTransaction + SimulationResult`) or `evaluate_raw` (from raw vectors).

## SimulationResult

```rust
pub struct SimulationResult {
    pub pre_balances: HashMap<String, u64>,
    pub post_balances: HashMap<String, u64>,
}
```

The `DrainCheck` rule on the simulated path compares `pre_balances` vs `post_balances` to detect any lamport drain from any account, not just explicit transfers.

## Instruction Data Parsing

The evaluator parses two instruction formats internally:

**System Program Transfer** (bincode, 4-byte discriminant):
```
[0x02, 0x00, 0x00, 0x00, lamports: u64 LE]   // Transfer
[0x00, 0x00, 0x00, 0x00, lamports: u64 LE]   // CreateAccount (also moves lamports)
```

**ComputeBudget** (1-byte discriminants, not bincode):
```
[0x02, units: u32 LE]       // SetComputeUnitLimit
[0x03, microlamports: u64 LE] // SetComputeUnitPrice
```
