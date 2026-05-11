# sak-sdk — Agent Developer API

The high-level interface that AI agents use to submit transactions, manage compressed state, and subscribe to on-chain events. Wraps all three SAK pillars into one `Kernel` struct.

```
Cargo.toml: sak-sdk = { path = "../crates/sak-sdk" }
```

## Kernel

```rust
pub struct Kernel {
    pub config: KernelConfig,
    // private: guardian, reflex, state
}
```

### Kernel::new

```rust
pub fn new(config: KernelConfig) -> Result<Self>
```

Create a `Kernel` shell. Call `.with_guardian()` before `submit()`.

### Kernel::with_guardian

```rust
pub fn with_guardian(self, rules_path: &str) -> Result<Self>
```

Load Guardian rules from a YAML file. Returns `Err` if the file is missing or malformed.

### Kernel::with_reflex

```rust
pub fn with_reflex(
    self,
    endpoint: &str,
    x_token: Option<String>,
) -> (Self, sak_reflex::EventRouter)
```

Attach the Reflex Engine (Yellowstone Geyser subscriber). Returns the `EventRouter` for registering callbacks.

### Kernel::with_state

```rust
pub fn with_state(self) -> Result<Self>
```

Attach ZK-compressed state storage (Pillar 3).

### Kernel::submit

```rust
pub fn submit(
    &mut self,
    tx: &VersionedTransaction,
    meta: &TxMeta,
) -> Decision
```

**The primary safety check.** Simulates the transaction in LiteSVM, evaluates all rules, returns `Decision::Allow` or `Decision::Reject { rule, reason }`. If Guardian was not configured, returns `Reject` (fail-closed).

### Kernel::state

```rust
pub fn state(&mut self) -> Option<&mut ZkState>
```

Access the state manager for reading/writing agent state.

### Kernel::reflex_config

```rust
pub fn reflex_config(&self) -> sak_reflex::ReflexConfig
```

Derive Yellowtone config from the kernel's geyser endpoint and API key.

### Kernel::start_reflex

```rust
pub async fn start_reflex(
    self,
    filter: sak_reflex::SubscribeFilter,
) -> Result<()>
```

Start the Reflex Engine subscriber loop.

## KernelConfig

```rust
#[derive(Clone, Debug, Default)]
pub struct KernelConfig {
    pub geyser_endpoint: Option<String>,
    pub helius_api_key: Option<String>,
    pub rules_path: Option<String>,
}
```

### KernelConfig::rules_path_or_default

```rust
pub fn rules_path_or_default(&self) -> String
```

Returns `self.rules_path` or `"rules.yaml"`.

## Shared Types (from `sak-core`)

### Decision

```rust
pub enum Decision {
    Allow,
    Reject { rule: String, reason: String },
}
```

The verdict returned by `submit()` and `evaluate()`. Variant `Allow` means the transaction passed all rules. `Reject` includes the rule name and a human-readable reason.

### TxMeta

```rust
pub struct TxMeta {
    pub slippage_bps: Option<u64>,
    pub description: Option<String>,
}
```

Intent metadata supplied alongside the transaction. Rules like `SlippageCheck` operate on `slippage_bps` rather than raw instruction bytes.

### ChainEvent

```rust
pub enum ChainEvent {
    AccountChanged { slot: u64, pubkey: String, lamports: u64 },
    ProgramInvoked { slot: u64, program_id: String },
    SlotUpdate { slot: u64, parent: Option<u64>, status: i32 },
    ShredEntry { slot: u64, data: Vec<u8> },
}
```

Typed on-chain events emitted by the Reflex Engine from the Yellowstone Geyser stream.

## Example

```rust
use sak_sdk::{Kernel, KernelConfig};
use sak_core::{Decision, TxMeta};

let config = KernelConfig {
    rules_path: Some("rules.yaml".into()),
    ..Default::default()
};

let mut kernel = Kernel::new(config)?
    .with_guardian("rules.yaml")?;

let decision = kernel.submit(&tx, &TxMeta {
    slippage_bps: Some(50),
    description: Some("swap 0.1 SOL to USDC".into()),
});

match decision {
    Decision::Allow => broadcast("Transaction approved"),
    Decision::Reject { rule, reason } => {
        warn!("Blocked by {rule}: {reason}");
    }
}
```
