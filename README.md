# SAK — Solana Agent Kernel

> Give AI agents same-slot reflexes, a pre-sign kill switch, and 1000× cheaper state storage — in one Rust kernel that plugs under any existing agent framework.

[![Integrate in 60 seconds →](https://img.shields.io/badge/Integrate-60_seconds-7c3aed)](INTEGRATE.md)

**SAK** is a Rust middleware kernel that sits between an LLM-driven agent and the Solana blockchain. It is the **execution and safety layer** — not the AI, not the blockchain, not the agent framework.

---

## What SAK Does

| Pillar | Component | What It Does |
|--------|-----------|--------------|
| **Pillar 2** | **Guardian** (✅ Complete) | Simulates every transaction in LiteSVM before signing. Blocks malicious tx with **zero on-chain cost**. |
| **Pillar 1** | **Reflex Engine** (✅ Complete) | Subscribes to Yellowstone Geyser push streams. Reacts to on-chain events within the same slot. |
| **Pillar 3** | **ZK State** (✅ Complete) | Stores agent state in Light Protocol ZK-compressed accounts. 100–1000× cheaper rent. |
| **SDK** | **sak-sdk** (✅ Complete) | Public API for agent developers. Simple `submit()` interface. |

---

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) ≥ 18 (for demo UI)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli) (optional, for deployment)

### Build Everything

```bash
git clone https://github.com/BALAJI-SK/sak.git
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

The demo shows the Guardian blocking malicious LLM-generated transactions in real time with a beautiful design system.

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
VITE v6.4.2 ready in 553 ms
➜  Local:   http://localhost:3000/
```

**Terminal 3 — Open browser:**
```
Navigate to http://localhost:3000
```

### What You'll See

The UI features a **three-panel layout** with the Claude design system:

```
┌─────────────────────────────────────────────────┐
│  SAK Guardian                              │
│  Every transaction simulated before signing    │
├─────────────────────────────────────────────┤
│  [Flow]  [Live Trace]  [Transaction Log]   │
│                                             │
│  Agent → Guardian → Solana                  │
│  BLOCKED: 106  ALLOWED: 14                 │
│                                             │
│  [BLOCKED] 99% Slippage Swap               │
│  Rule: max_slippage — 9900bps > 200bps    │
│  Prevented loss: $498.50                   │
│                                             │
│  [ALLOWED] Valid USDC Transfer              │
│  All 7 rules passed ✓                      │
└─────────────────────────────────────────────┘
```

- **RED** = BLOCKED (with rule name, reason, and prevented loss)
- **GREEN** = ALLOWED (with simulation confirmation)
- **Severity pills**: CRITICAL / HIGH / MEDIUM / LOW
- **Simulation time**: Displayed in ms (typically 28-60ms)
- **Feedback system**: Rate decisions 1-5 stars or Wrong/Correct

---

## Project Structure

```
SAK/
├── Cargo.toml                     # workspace root
├── README.md                     # this file
├── SAK.md                       # full project context
├── SAK_BUILD_PHASES.md          # detailed build guide
├── rules.yaml                    # Guardian rule definitions
│
├── crates/
│   ├── sak-core/                # shared types + errors (✅)
│   ├── sak-guardian/            # Pillar 2: rule engine (✅)
│   │   ├── src/
│   │   │   ├── lib.rs          # Guardian API + parse_simulation_error()
│   │   │   ├── rules.rs        # rule definitions (YAML)
│   │   │   ├── evaluator.rs    # rule evaluation logic
│   │   │   └── simulator.rs    # LiteSVM simulation
│   │   └── tests/
│   │       └── evil_corpus.rs  # 20 malicious tx patterns
│   ├── sak-reflex/             # Pillar 1: Geyser subscriber (✅)
│   ├── sak-state/               # Pillar 3: ZK state (✅)
│   ├── sak-sdk/                # public agent-facing API (✅)
│   └── sak-bin/                # CLI daemon (✅)
│
└── demo/
    ├── README.md                  # Design system documentation
    ├── colors_and_type.css       # Design tokens (colors, type, radii)
    ├── assets/                   # Brand marks (sak-shield.svg, sak-wordmark.svg)
    ├── fonts/                    # Surgena font family (light/regular/medium/semibold/bold)
    ├── ui_kits/dashboard/        # Reference implementation (JSX + HTML)
    ├── preview/                  # HTML preview cards
    ├── tx-generator/            # generates evil + valid transactions (70/30 mix)
    ├── race-server/             # WebSocket server (broadcasts to UI)
    └── race-ui/                # React UI (live safety log)
        ├── src/
        │   ├── App.tsx         # Three-panel layout with Surgena font
        │   ├── index.css       # Design tokens + Tailwind
        │   └── main.tsx        # Entry point
        ├── tailwind.config.js   # Tailwind configuration
        ├── postcss.config.js    # PostCSS configuration
        └── tsconfig.json       # TypeScript configuration
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
let decision: Decision = guardian.evaluate(&transaction, &meta);

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
      - whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc    # Orca Whirlpool
      - 11111111111111111111111111111111                # System program
      - TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA    # SPL Token
      - ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bJ     # Associated Token Account
      - ComputeBudget111111111111111111111111111111       # Compute Budget

  - name: max_account_drain
    type: drain_check
    max_lamports: 1000000000  # 1 SOL max transfer

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

## Evil Corpus — 20 Malicious Patterns Blocked

| # | Attack Pattern | Rule Fired | Severity |
|---|----------------|-----------|----------|
| 1 | 99% slippage swap | `max_slippage` | critical |
| 2 | Wrong token mint (fake USDC) | `allowed_programs` | high |
| 3 | Drain entire SOL balance | `max_account_drain` | critical |
| 4 | Unknown program ID | `allowed_programs` | high |
| 5 | Transfer to attacker wallet | `max_account_drain` | critical |
| 6 | Slippage set to u64::MAX | `max_slippage` | critical |
| 7 | Jupiter route through unlisted pool | `allowed_programs` | high |
| 8 | Zero-amount dust attack | `min_transfer_value` | low |
| 9 | Account substitution | `max_account_drain` | critical |
| 10 | Balance underflow | `max_account_drain` | critical |
| 11 | Excessive compute units | `max_compute_units` | medium |
| 12 | Reentrancy-style CPI loop | `allowed_programs` | high |
| 13 | Fake system program ID | `allowed_programs` | high |
| 14 | Multiple drain instructions | `max_account_drain` | critical |
| 15 | Slippage bypass via CPI | `max_slippage` | critical |
| 16 | Token account closed mid-tx | `allowed_programs` | high |
| 17 | Priority fee 100× normal | `max_priority_fee` | medium |
| 18 | Memo field injection | `allowed_programs` | high |
| 19 | Transaction with 30+ accounts | `max_accounts` | medium |
| 20 | Unverified program (mainnet fail) | `allowed_programs` | high |

---

## Feedback System

The demo includes a **live feedback system** that allows users to rate Guardian decisions:

- **Star ratings**: 1-5 stars on each blocked transaction
- **Quick buttons**: "Wrong" (1 star) / "Correct" (5 stars)
- **Backend storage**: Feedback stored in memory via `FeedbackStore`
- **Summary endpoint**: `GET /feedback/summary` returns:
  ```json
  {
    "total": 18,
    "correct": 14,
    "wrong": 4,
    "accuracy": 77.8
  }
  ```

---

## Design System

The UI uses a **custom design system** inspired by monitoring consoles (Bloomberg Terminal energy):

### Typography
- **Surgena** (light/regular/medium/semibold/bold + italics) — Display + UI
- **JetBrains Mono** (400/500/700) — Code, numbers, addresses

### Color Palette

| Token | Hex | Use |
|---|---|---|
| `bg` | `#0a0a0f` | Page background (near black, blue-shifted) |
| `surface` | `#12121a` | Cards, panels |
| `border` | `#1e1e2e` | All borders, dividers |
| `green` | `#00ff88` | ALLOWED, system-active, brand |
| `red` | `#ff3366` | BLOCKED, critical |
| `orange` | `#ff9900` | High severity, warning |
| `purple` | `#7c3aed` | AI agent node, links, accent |

### Animation
- **Transitions**: 180–300ms, `cubic-bezier(0.2, 0.8, 0.2, 1)`
- **Slide-in**: New log cards slide from right (280ms)
- **Glow**: Red/green border glow on appear (1000ms)
- **Pulse**: System Active dot pulses every 2s

### Icons
- **Lucide** — outline style, 1.5px stroke, 24×24 default
- Loaded via CDN: `https://unpkg.com/lucide@latest/dist/umd/lucide.js`

---

## Current Status

| Phase | Component | Status |
|-------|------------|--------|
| 0 | Workspace scaffold | ✅ Complete |
| 1 | sak-core (shared types) | ✅ Complete |
| 2 | sak-guardian (Pillar 2) | ✅ Complete |
| 3 | Demo UI (WebSocket + React) | ✅ Complete |
| 4 | sak-reflex (Pillar 1) | ✅ Complete |
| 5 | sak-state (Pillar 3) | ✅ Complete |
| 6 | sak-sdk (public API) | ✅ Complete |
| 7 | Feedback System | ✅ Complete |
| 8 | UI Design System | ✅ Complete |
| 9 | Demo Recording | ⬜ Pending |
| 10 | Deployment + Submission | ⬜ Pending |

**Hackathon:** Colosseum Frontier  
**Deadline:** May 11, 2026 (5 days remaining)

---

## Technology Stack

### Core
- **Rust** — systems language for performance
- **LiteSVM** — local Solana VM for transaction simulation
- **Tokio** — async runtime

### Guardian (Pillar 2)
- **serde / serde_yaml** — rule configuration
- **tracing** — structured logging
- **litesvm** — Solana VM simulation

### Reflex Engine (Pillar 1)
- **yellowstone-grpc** — Geyser subscriber
- **tokio-stream** — stream processing

### ZK State (Pillar 3)
- **light-protocol** — ZK compression
- **solana-program** — on-chain program

### Demo UI
- **React + TypeScript** — frontend
- **Vite** — build tool (v6.4.2)
- **Tailwind CSS** — utility-first styling (v3)
- **Surgena Font** — custom brand typography
- **Lucide Icons** — outline icon set
- **Axum + tokio-tungstenite** — WebSocket server

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
0:30-0:60  Show 3-4 blocked transactions with rule names + prevented loss
0:60-0:80  Show allowed transaction going through
0:80-0:90  Call to action: "Ship agents that can't be used against you"
```

### Key Metrics
- **20/20** evil corpus tests passing
- **28-60ms** average simulation time
- **70/30** blocked/allowed transaction mix
- **7** active rules in Guardian
- **1000×** cheaper than on-chain simulation

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
- **Design System:** See `demo/README.md` for UI design documentation

---

**Built with ❤️ for the Solana ecosystem**
