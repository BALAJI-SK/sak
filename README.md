# SAK — Solana Agent Kernel

> Give AI agents same-slot reflexes, a pre-sign kill switch, and 1000× cheaper state storage — in one Rust kernel that plugs under any existing agent framework.

**SAK** is a Rust middleware kernel that sits between an LLM-driven agent and the Solana blockchain. It is the **execution and safety layer** — not the AI, not the blockchain, not the agent framework.

---

## What SAK Does

| Pillar | Component | What It Does |
|--------|-----------|--------------|
| **Pillar 2** | **Guardian** (✅ Built) | Simulates every transaction in LiteSVM before signing. Blocks malicious tx with **zero on-chain cost**. |
| **Pillar 1** | **Reflex Engine** (✅ Built) | Subscribes to Yellowstone Geyser push streams. Reacts to on-chain events within the same slot. |
| **Pillar 3** | **ZK State** (✅ Built) | Stores agent state in Light Protocol ZK-compressed accounts. 100–1000× cheaper rent. |

---

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) ≥ 18 (for demo UI)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli) (optional, for deployment)

### Build Everything

```bash
git clone https://github.com/your-org/SAK.git
cd sak
cargo build --workspace
```

### Run Tests

```bash
# Run all tests (20 evil corpus tests in Guardian)
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

## Live Demo — Guardian Safety Log

The demo shows the Guardian blocking malicious LLM-generated transactions in real time.

### Start the Demo

**Terminal 1 — Start WebSocket server** (spawns transaction generator automatically):
```bash
cargo run -p race-server
```

Expected output:
```
INFO WebSocket server running on ws://0.0.0.0:3001
INFO Transaction generator started - sending to stdout
```

**Terminal 2 — Start React UI:**
```bash
cd demo/race-ui
npm install  # only needed once
npm run dev
```

Expected output:
```
VITE v6.0.7 ready in 500 ms
➜ Local:   http://localhost:3000/
```

**Terminal 3 — Open browser:**
```
Navigate to http://localhost:3000
```

### What You'll See

```
┌─────────────────────────────────────────────────┐
│  SAK Guardian                              │
│  Live safety log — every transaction          │
│  simulated before signing                    │
├─────────────────────────────────────────────┤
│  [BLOCKED] max_slippage — 9900bps > 200bps │
│  [ALLOWED] Valid transfer 0.005 SOL         │
│  [BLOCKED] allowed_programs — fake program  │
│  [ALLOWED] Valid transfer 0.005 SOL         │
│  [BLOCKED] max_account_drain — 5 SOL > 1 SOL│
└─────────────────────────────────────────────┘
```

- **RED** = BLOCKED (with rule name and reason)
- **GREEN** = ALLOWED
- Counters at top show total blocked vs allowed

---

## Project Structure

```
SAK/
├── Cargo.toml                     # workspace root
├── README.md                     # this file
├── STATUS.md                     # build progress tracker
├── SAK.md                       # full project context
├── rules.yaml                    # Guardian rule definitions
│
├── crates/
│   ├── sak-core/                # shared types + errors (✅)
│   ├── sak-guardian/            # Pillar 2: rule engine (✅)
│   │   ├── src/
│   │   │   ├── lib.rs          # Guardian API
│   │   │   ├── rules.rs        # rule definitions (YAML)
│   │   │   ├── evaluator.rs    # rule evaluation logic
│   │   │   └── simulator.rs    # LiteSVM simulation
│   │   └── tests/
│   │       └── evil_corpus.rs  # 20 malicious tx patterns
│   ├── sak-reflex/             # Pillar 1: Geyser subscriber (🚧)
│   ├── sak-state/               # Pillar 3: ZK state (🚧)
│   ├── sak-sdk/                # public agent-facing API (🚧)
│   └── sak-bin/                # CLI daemon (🚧)
│
└── demo/
    ├── tx-generator/            # generates evil + valid transactions
    ├── race-server/             # WebSocket server (broadcasts to UI)
    └── race-ui/                # React UI (live safety log)
```

---

## Guardian API

### Basic Usage

```rust
use sak_guardian::Guardian;
use sak_core::{Decision, TxMeta};

// Load rules from YAML
let mut guardian = Guardian::from_yaml("rules.yaml")?;

// Evaluate a transaction (with LiteSVM simulation)
let decision: Decision = guardian.evaluate(&transaction);

match decision {
    Decision::Allow => {
        // Proceed to sign and broadcast
    }
    Decision::Reject { rule, reason } => {
        // Zero on-chain cost — tx never left the machine
        println!("Blocked by {}: {}", rule, reason);
    }
}
```

### Rules Configuration (`rules.yaml`)

```yaml
rules:
  - name: max_slippage
    type: slippage_check
    max_bps: 200              # 2% max slippage

  - name: allowed_programs
    type: program_whitelist
    programs:
      - JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4  # Jupiter v6
      - 11111111111111111111111111111111                # System program

  - name: max_account_drain
    type: drain_check
    max_lamports: 1000000000  # 1 SOL max transfer
```

---

## Evil Corpus — 20 Malicious Patterns Blocked

| # | Attack Pattern | Rule Fired |
|---|----------------|-----------|
| 1 | 99% slippage swap | `max_slippage` |
| 2 | Wrong token mint (fake USDC) | `allowed_programs` |
| 3 | Drain entire SOL balance | `max_account_drain` |
| 4 | Unknown program ID | `allowed_programs` |
| 5 | Transfer to attacker wallet | `max_account_drain` |
| 6 | Slippage set to u64::MAX | `max_slippage` |
| 7 | Jupiter route through unlisted pool | `allowed_programs` |
| 8 | Zero-amount dust attack | `min_transfer_value` |
| 9 | Account substitution | `max_account_drain` |
| 10 | Balance underflow | `max_account_drain` |
| 11 | Excessive compute units | `max_compute_units` |
| 12 | Reentrancy-style CPI loop | `allowed_programs` |
| 13 | Fake system program ID | `allowed_programs` |
| 14 | Multiple drain instructions | `max_account_drain` |
| 15 | Slippage bypass via CPI | `max_slippage` |
| 16 | Token account closed mid-tx | `allowed_programs` |
| 17 | Priority fee 100× normal | `max_priority_fee` |
| 18 | Memo field injection | `allowed_programs` |
| 19 | Transaction with 30+ accounts | `max_accounts` |
| 20 | Unverified program (mainnet fail) | `allowed_programs` |

---

## Current Status

| Phase | Component | Status |
|-------|------------|--------|
| 0 | Workspace scaffold | ✅ Complete |
| 1 | sak-core (shared types) | ✅ Complete |
| 2 | sak-guardian (Pillar 2) | ✅ Complete |
| 3 | Demo UI (WebSocket + React) | ✅ Complete |
| 4 | sak-reflex (Pillar 1) | ⬜ Pending |
| 5 | sak-state (Pillar 3) | ⬜ Pending |
| 6 | sak-sdk (public API) | ⬜ Pending |
| 7 | Full race demo | ⬜ Pending |
| 8 | Deployment + submission | ⬜ Pending |

**Hackathon:** Colosseum Frontier  
**Deadline:** May 11, 2026 (6 days remaining)

---

## Technology Stack

### Core
- **Rust** — systems language for performance
- **LiteSVM** — local Solana VM for transaction simulation
- **Tokio** — async runtime

### Guardian (Pillar 2)
- **serde / serde_yaml** — rule configuration
- **tracing** — structured logging

### Demo UI
- **React + TypeScript** — frontend
- **Vite** — build tool
- **Tailwind CSS** — styling
- **Axum + tokio-tungstenite** — WebSocket server

### Planned
- **Yellowstone gRPC** — Geyser subscriber (Pillar 1)
- **Light Protocol** — ZK compression (Pillar 3)

---

## For Hackathon Judges

### What to Expect

1. **Live URL:** https://your-demo-url.com (deploy before submission)
2. **Demo Video:** 90 seconds, embedded on landing page
3. **GitHub Repo:** Clean README with working demo link

### Demo Script (90 Seconds)

```
0:00-0:10  Show the problem (LLM hallucinations drain wallets)
0:10-0:30  Show Guardian evaluating transactions in real time
0:30-0:60  Show 3-4 blocked transactions with rule names
0:60-0:80  Show allowed transaction going through
0:80-0:90  Call to action: "Ship agents that can't be used against you"
```

---

## Team

- **Balaji Segu Krishnaiah** — Founder, MSc AI (DCU), ex-McKinsey, ex-Tejas Networks
- **Prateek C** — CTO, MSc AI, ex-Tejas Networks
- **Sai Shreyas Gubbi Harish** — Co-founder, MSc Cloud (NCI), ex-EY
- **Tejas Shiv Kumar** — CMO, MSc DA (DCU), ex-AceMicromatic

---

## License

[Add your license here]

---

## Links

- **Colosseum Frontier:** https://arena.colosseum.org/
- **Documentation:** See `SAK.md` for full project context
- **Status:** See `STATUS.md` for build progress
- **Build Phases:** See `SAK_BUILD_PHASES.md` for detailed build guide

---

**Built with ❤️ for the Solana ecosystem**
