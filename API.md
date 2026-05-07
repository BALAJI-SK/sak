# SAK API Reference

> **SAK** (Solana Agent Kernel) — Give AI agents same-slot reflexes, a pre-sign kill switch, and 1000× cheaper state storage.

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Guardian API](#guardian-api)
4. [Reflex Engine API](#reflex-engine-api)
5. [ZK State API](#zk-state-api)
6. [SDK API](#sdk-api)
7. [WebSocket API](#websocket-api)
8. [REST API](#rest-api)
9. [Error Handling](#error-handling)
10. [Examples](#examples)

---

## Overview

SAK is a Rust middleware kernel that sits between an LLM-driven agent and the Solana blockchain. It provides:

- **Pillar 2 - Guardian**: Simulates transactions in LiteSVM before signing
- **Pillar 1 - Reflex Engine**: Subscribes to Geyser streams for same-slot reactions
- **Pillar 3 - ZK State**: Stores agent state in ZK-compressed accounts
- **SDK**: Public API for agent developers

### Architecture

```
┌─────────────────────────────────────────────┐
│         LLM-Driven Agent (Python/Node)      │
└───────────────┬─────────────────────────────┘
                │ submit()
                ▼
┌─────────────────────────────────────────────┐
│         SAK SDK (Pillar 4)                 │
└───────────────┬─────────────────────────────┘
                │ evaluate()
                ▼
┌─────────────────────────────────────────────┐
│    Guardian (Pillar 2)                     │
│    • Rule Engine (7 active rules)           │
│    • LiteSVM Simulation (28-60ms)           │
└───────────────┬─────────────────────────────┘
                │ allow / reject
                ▼
┌─────────────────────────────────────────────┐
│         Solana Blockchain                    │
└─────────────────────────────────────────────┘
```

---

## Installation

### Prerequisites

- **Rust** (stable toolchain) — [Install](https://rustup.rs/)
- **Node.js** ≥ 18 (for demo UI) — [Install](https://nodejs.org/)
- **Solana CLI** (optional) — [Install](https://docs.solana.com/cli/install-solana-cli)

### Build from Source

```bash
git clone https://github.com/BALAJI-SK/sak.git
cd sak
cargo build --workspace
```

### Run Tests

```bash
# Run all workspace tests (20 evil corpus patterns)
cargo test --workspace

# Run only Guardian tests
cargo test -p sak-guardian
```

Expected output:
```
running 20 tests
test blocks_99_percent_slippage          ... ok
test blocks_wrong_token_mint             ... ok
... (18 more)
test result: ok. 20 passed; 0 failed
```

---

## Guardian API

The Guardian is the core safety layer that simulates transactions before signing.

### `Guardian::from_yaml`

Load rules from a YAML configuration file.

```rust
use sak_guardian::Guardian;

let guardian: Guardian = Guardian::from_yaml("rules.yaml")?;
```

**Parameters:**
- `path` (`impl AsRef<Path>`) — Path to the YAML rules file

**Returns:** `Result<Guardian>`

**Example:**
```rust
let mut guardian = Guardian::from_yaml("rules.yaml")
    .expect("Failed to load rules.yaml");
```

---

### `Guardian::from_yaml_with_svm`

Load rules and use an existing LiteSVM instance (for sharing state with transaction generator).

```rust
use sak_guardian::Guardian;
use litesvm::LiteSVM;

let svm = LiteSVM::new();
let mut guardian = Guardian::from_yaml_with_svm("rules.yaml", svm)?;
```

**Parameters:**
- `path` (`impl AsRef<Path>`) — Path to YAML rules file
- `svm` (`LiteSVM`) — Existing LiteSVM instance

**Returns:** `Result<Guardian>`

---

### `Guardian::evaluate`

Simulate and evaluate a transaction. Returns a `Decision` after running LiteSVM simulation + rule checks.

```rust
use sak_guardian::Guardian;
use sak_core::{Decision, TxMeta};
use solana_transaction::versioned::VersionedTransaction;

let decision: Decision = guardian.evaluate(&transaction, &meta);
```

**Parameters:**
- `tx` (`&VersionedTransaction`) — The transaction to evaluate
- `meta` (`&TxMeta`) — Intent metadata (slippage, description)

**Returns:** `Decision`

**Decision Types:**

```rust
pub enum Decision {
    Allow,                      // Transaction is safe to sign
    Reject { rule: String, reason: String },  // Blocked with reason
}
```

**Example:**
```rust
let meta = TxMeta {
    slippage_bps: Some(100),  // 1% slippage
    description: Some("Swap 100 USDC".into()),
};

match guardian.evaluate(&tx, &meta) {
    Decision::Allow => {
        println!("✓ Transaction allowed");
        // Proceed to sign and broadcast
    }
    Decision::Reject { rule, reason } => {
        println!("✗ Blocked by {}: {}", rule, reason);
        // Zero on-chain cost — tx never left the machine
    }
}
```

---

### `Guardian::evaluate_raw`

Evaluate a transaction from raw account keys + instruction data (without a full transaction object).

```rust
let view = TxView::from_raw(account_keys, instructions);
let decision = evaluate(&rules, &view, &meta);
```

**Parameters:**
- `account_keys` (`Vec<String>`) — Base-58 encoded public keys
- `instructions` (`&[(u8, &[u8])]`) — Program ID index + instruction data pairs
- `meta` (`&TxMeta`) — Intent metadata

**Returns:** `Decision`

---

### `TxMeta`

Intent metadata supplied by the agent alongside the transaction.

```rust
pub struct TxMeta {
    /// Slippage tolerance in basis points (1 bps = 0.01%)
    pub slippage_bps: Option<u64>,

    /// Human-readable description of the intended action
    pub description: Option<String>,
}
```

**Example:**
```rust
let meta = TxMeta {
    slippage_bps: Some(200),  // 2% max slippage
    description: Some("Valid swap via Jupiter".into()),
};
```

---

## Reflex Engine API

Subscribes to Yellowstone Geyser push streams for same-slot event reactions.

### `ReflexEngine::new`

Create a new Reflex Engine instance.

```rust
use sak_reflex::ReflexEngine;

let engine = ReflexEngine::new("http://localhost:10000")?;
```

**Parameters:**
- `endpoint` (`&str`) — Yellowstone gRPC endpoint URL

**Returns:** `Result<ReflexEngine>`

---

### `ReflexEngine::subscribe`

Subscribe to on-chain events.

```rust
let events = engine.subscribe().await?;

while let Some(event) = events.next().await {
    match event.kind {
        EventKind::AccountChanged { pubkey, lamports } => {
            println!("Account {} changed: {} lamports", pubkey, lamports);
        }
        EventKind::ProgramInvoked { program_id } => {
            println!("Program {} invoked", program_id);
        }
    }
}
```

---

### `ChainEvent`

On-chain event produced by the Reflex Engine.

```rust
pub struct ChainEvent {
    pub slot: u64,
    pub kind: EventKind,
}

pub enum EventKind {
    AccountChanged { pubkey: String, lamports: u64 },
    ProgramInvoked { program_id: String },
}
```

---

## ZK State API

Stores agent state in Light Protocol ZK-compressed accounts (100-1000× cheaper rent).

### `ZkState::new`

Create a new ZK State instance.

```rust
use sak_state::ZkState;

let state = ZkState::new().await?;
```

**Returns:** `Result<ZkState>`

---

### `ZkState::store`

Store agent state in a ZK-compressed account.

```rust
state.store(agent_id, &state_data).await?;
```

**Parameters:**
- `agent_id` (`&str`) — Unique agent identifier
- `data` (`&[u8]`) — Serialized state data

---

### `ZkState::load`

Load agent state from ZK-compressed account.

```rust
let data = state.load(agent_id).await?;
```

**Returns:** `Result<Vec<u8>>`

---

## SDK API

Public API for agent developers. Simple `submit()` interface.

### `Kernel::new`

Create a new SAK Kernel instance.

```rust
use sak_sdk::Kernel;

let mut kernel = Kernel::new()?;
```

**Returns:** `Result<Kernel>`

---

### `Kernel::with_guardian`

Attach a Guardian with rules from YAML.

```rust
let kernel = kernel.with_guardian("rules.yaml")?;
```

**Parameters:**
- `rules_path` (`impl AsRef<Path>`) — Path to rules.yaml

**Returns:** `Result<Kernel>`

---

### `Kernel::submit`

Submit a transaction for evaluation and signing.

```rust
let result = kernel.submit(transaction, meta).await?;
```

**Parameters:**
- `transaction` (`VersionedTransaction`) — Transaction to submit
- `meta` (`TxMeta`) — Intent metadata

**Returns:** `Result<Decision>`

**Example:**
```rust
use sak_sdk::Kernel;
use sak_core::{Decision, TxMeta};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut kernel = Kernel::new()?;
    let kernel = kernel.with_guardian("rules.yaml")?;

    // Agent generates a transaction
    let tx = build_transaction();
    let meta = TxMeta {
        slippage_bps: Some(100),
        description: Some("Swap 100 USDC for SOL".into()),
    };

    match kernel.submit(tx, meta).await? {
        Decision::Allow => {
            println!("✓ Transaction approved — signing and broadcasting");
        }
        Decision::Reject { rule, reason } => {
            println!("✗ Blocked by {}: {}", rule, reason);
        }
    }

    Ok(())
}
```

---

## WebSocket API

Real-time transaction log stream for the demo UI.

### Connection

```javascript
const ws = new WebSocket("ws://localhost:3001/ws");
```

### Message Format

Each message is a JSON object:

```json
{
  "id": "uuid-v4-string",
  "timestamp": "2026-05-07T16:21:55Z",
  "decision": "rejected",
  "rule": "max_slippage",
  "reason": "9900bps exceeds maximum 200bps",
  "attack_type": "99% Slippage Swap",
  "description": "Agent tried to swap 100 USDC with 99% slippage tolerance",
  "severity": "critical",
  "simulated_loss_usd": 498.50,
  "simulation_time_ms": 43
}
```

**Allowed transaction:**
```json
{
  "id": "uuid-v4-string",
  "timestamp": "2026-05-07T16:22:01Z",
  "decision": "allowed",
  "attack_type": "Valid Swap",
  "description": "Agent executed valid swap with 1% slippage tolerance",
  "severity": "none",
  "simulation_time_ms": 32
}
```

### Severity Levels

| Level | Description | Color |
|-------|-------------|-------|
| `critical` | Drain attacks, 99% slippage | Red (#ff3366) |
| `high` | Unknown programs, account below rent | Orange (#ff9900) |
| `medium` | Excessive compute/priority fees | Yellow (#ffd700) |
| `low` | Zero amount transfers | Gray (#8888aa) |
| `none` | Valid transactions | Green (#00ff88) |

---

## REST API

### `POST /feedback`

Submit user feedback for a Guardian decision.

**Request:**
```json
{
  "timestamp": "2026-05-07T16:21:55Z",
  "decision": "rejected",
  "rule": "max_slippage",
  "description": "99% Slippage Swap",
  "stars": 1,
  "verdict": "Wrong"
}
```

**Response:** `"recorded"` (HTTP 200)

**Star Rating Mapping:**
- 1-2 stars → `Wrong`
- 3 stars → `Neutral`
- 4-5 stars → `Correct`

---

### `GET /feedback/summary`

Get feedback statistics.

**Response:**
```json
{
  "total": 18,
  "correct": 14,
  "wrong": 4,
  "accuracy": 77.8
}
```

---

## Error Handling

### Guardian Errors

| Error Type | Description | Human-Readable Message |
|------------|-------------|----------------------|
| `InsufficientFundsForRent` | Account would go below rent minimum | "Insufficient funds — transaction would leave account below rent minimum" |
| `InsufficientFunds` | Not enough balance | "Insufficient balance to complete transaction" |
| `InvalidAccountData` | Wrong token address | "Invalid account data — possible wrong token address" |
| `ProgramFailedToComplete` | Transaction would revert | "Program execution failed — transaction would revert on-chain" |

### Example: Handling Simulation Errors

```rust
match guardian.evaluate(&tx, &meta) {
    Decision::Reject { rule, reason } => {
        // reason is already human-readable (parsed by parse_simulation_error())
        println!("Blocked: {}", reason);
    }
    Decision::Allow => {
        // Safe to proceed
    }
}
```

---

## Examples

### Example 1: Basic Guardian Usage

```rust
use sak_guardian::Guardian;
use sak_core::{Decision, TxMeta};

fn main() -> anyhow::Result<()> {
    // Load rules
    let mut guardian = Guardian::from_yaml("rules.yaml")?;

    // Create transaction and metadata
    let tx = build_transaction();  // Your transaction builder
    let meta = TxMeta {
        slippage_bps: Some(50),  // 0.5% slippage
        description: Some("Transfer 0.5 SOL".into()),
    };

    // Evaluate
    match guardian.evaluate(&tx, &meta) {
        Decision::Allow => {
            println!("✓ Allowed — signing...");
            // sign_and_broadcast(&tx)?;
        }
        Decision::Reject { rule, reason } => {
            println!("✗ Blocked by {}: {}", rule, reason);
        }
    }

    Ok(())
}
```

---

### Example 2: Running the Demo Server

```bash
# Terminal 1: Start WebSocket server (spawns transaction generator)
cargo run -p race-server

# Expected output:
# INFO WebSocket server running on ws://0.0.0.0:3001
# INFO Transaction generator started - sending to stdout
```

```bash
# Terminal 2: Start React UI
cd demo/race-ui
npm install  # only needed once
npm run dev

# Expected output:
# VITE v6.4.2 ready in 553ms
# ➜  Local:   http://localhost:3000/
```

---

### Example 3: Custom Transaction Generator

```rust
use sak_guardian::Guardian;
use solana_transaction::Transaction;
use solana_message::Message;

struct TxFactory {
    svm: litesvm::LiteSVM,
    payer: Keypair,
}

impl TxFactory {
    fn generate_slippage_attack(&self) -> (Transaction, TxMeta) {
        let recipient = Address::new_unique();
        let ix = transfer(&self.payer.pubkey(), &recipient, 1_000_000);
        let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
        let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());

        let meta = TxMeta {
            slippage_bps: Some(9900),  // 99% slippage!
            description: Some("99% Slippage Swap".into()),
        };

        (tx, meta)
    }
}

// In main loop:
let (tx, meta) = factory.generate_slippage_attack();
let decision = guardian.evaluate(&tx.into(), &meta);
// → Decision::Reject { rule: "max_slippage", reason: "..." }
```

---

### Example 4: Feedback Integration

```javascript
// In React UI
const sendFeedback = async (index, stars) => {
  const entry = log[index];

  const verdict = stars <= 2 ? "Wrong" :
                  stars >= 4 ? "Correct" : "Neutral";

  const response = await fetch("http://localhost:3001/feedback", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      timestamp: entry.timestamp,
      decision: entry.decision,
      rule: entry.rule,
      description: entry.description,
      stars,
      verdict,
    }),
  });

  if (response.ok) {
    console.log("Feedback recorded!");
  }
};
```

---

## Rules Configuration

The Guardian uses a YAML file to define rules. See `rules.yaml` for the full configuration.

### Available Rules

| Rule Type | YAML `type` | Description |
|-----------|---------------|-------------|
| Slippage Check | `slippage_check` | Reject if slippage > max_bps |
| Program Whitelist | `program_whitelist` | Reject if program not in list |
| Drain Check | `drain_check` | Reject if transfer > max_lamports |
| Account Count | `account_count_check` | Reject if accounts > max_count |
| Compute Units | `compute_units_check` | Reject if units > max_units |
| Priority Fee | `priority_fee_check` | Reject if fee > max_microlamports |
| Min Transfer | `min_transfer_lamports` | Reject if transfer < min_lamports |

### Example `rules.yaml`

```yaml
rules:
  - name: max_slippage
    type: slippage_check
    max_bps: 200              # 2% max slippage

  - name: allowed_programs
    type: program_whitelist
    programs:
      - JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4   # Jupiter v6
      - 11111111111111111111111111111111                 # System program

  - name: max_account_drain
    type: drain_check
    max_lamports: 1000000000  # 1 SOL max transfer
```

---

## Performance Metrics

| Metric | Value |
|--------|-------|
| Evil corpus tests | 20/20 passing |
| Average simulation time | 28-60ms |
| Transaction mix (demo) | ~70% blocked, ~30% allowed |
| Active rules | 7 |
| Cost savings | 1000× cheaper than on-chain simulation |
| UI build size | 157 KB JS + 4.85 KB CSS (gzipped) |

---

## License

[Add your license here]

---

## Links

- **GitHub Repository:** https://github.com/BALAJI-SK/sak
- **Colosseum Frontier:** https://arena.colosseum.org/
- **Documentation:** See `SAK.md` for full project context
- **Build Phases:** See `SAK_BUILD_PHASES.md` for detailed build guide
- **Design System:** See `demo/README.md` for UI documentation

---

**Built with ❤️ for the Solana ecosystem**
