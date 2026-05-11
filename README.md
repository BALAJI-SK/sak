# SAK — Solana Agent Kernel

> The execution kernel for Solana AI agents — pre-sign safety, oracle-grade reflexes, and 1000× cheaper on-chain state in one Rust crate.

[![Integrate in 60 seconds →](https://img.shields.io/badge/Integrate%20in%2060%20seconds%20%E2%86%92-7c3aed)](INTEGRATE.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-f4662c)](https://rustup.rs/)

SAK is a **Rust execution kernel** that plugs under any Solana AI agent framework. Every transaction is simulated in LiteSVM before the agent ever signs — eliminating capital waste from failed and malicious transactions at zero on-chain cost.

## Quick Start

```bash
git clone https://github.com/BALAJI-SK/sak.git
cd sak
cargo build --workspace
cargo test -p sak-guardian  # 25 tests: 24 evil-corpus + 1 pack-load
```

## What SAK Does

| Pillar | Component | Status | What It Does |
|--------|-----------|--------|--------------|
| **Guardian** | `sak-guardian` | ✅ Complete | Simulates every tx in LiteSVM and evaluates against a **2,010-rule** indexed policy set (8 detector types · 4 rule packs). Zero on-chain cost, <50ms. |
| **Oracle** | `sak-reflex` | ✅ Complete | Yellowstone Geyser push oracle — emits `ChainEvent` into an async channel within the same slot. No polling, no RPC overhead. |
| **State** | `sak-state` | 🔧 Stub | In-memory HashMap. Light Protocol ZK-compression is the next milestone — 100–1000× cheaper than standard accounts. API surface stable. |
| **SDK** | `sak-sdk` | ✅ Complete | `Kernel` struct wraps all pillars. One `submit()` call integrates SAK under any agent framework. |

## Architecture

```
AI Agent (LLM intent)
        │
        ▼
┌─────────────────────────────────────────────┐
│  Guardian — sak-guardian                    │
│                                             │
│   ① LiteSVM pre-sign simulation             │
│   ② Indexed rule dispatch                   │
│       · 2,003 blocked_program (O(1) lookup) │
│       · 7   global detectors                │
│   ③ Decision: Allow | Reject{rule,reason}   │
│                                             │
│  <50ms · off-chain · zero on-chain cost     │
└──────────────────┬──────────────────────────┘
                   │ ALLOW
                   ▼
            Solana Blockchain

Background push oracle (same-slot):
┌─────────────────────────────┐
│  Reflex Engine              │  Yellowstone Geyser → ChainEvent channel
│  sak-reflex                 │  SlotUpdate · AccountChanged · ProgramInvoked
└─────────────────────────────┘
```

## Guardian Rule Engine

Rules are loaded from YAML packs at startup and indexed once. Per-transaction evaluation is `O(programs_touched + global_rules)` — a 2,000-entry blocklist costs the same as a 20-entry one for any given tx.

### Detector types

| Type | What it checks |
|------|---------------|
| `slippage_check` | Agent-declared slippage cap (bps) |
| `program_whitelist` | Reject if any instruction invokes an unlisted program |
| `blocked_program` | Reject if a specific program id appears in the tx (negative list) |
| `drain_check` | Reject system-program transfers exceeding `max_lamports` |
| `account_count_check` | Reject txs referencing too many accounts |
| `compute_units_check` | Cap ComputeBudget `SetComputeUnitLimit` |
| `priority_fee_check` | Cap ComputeBudget `SetComputeUnitPrice` (microlamports) |
| `min_transfer_lamports` | Reject dust transfers |

### Shipped rule packs (`packs/`)

| Pack | Source | Rules |
|------|--------|-------|
| `defaults.yaml` | Hand-written baseline | 6 |
| `solana-core.yaml` | 41 curated mainnet programs | 1 (whitelist) |
| `exploits-blocklist.yaml` | Curated scam program ids | 3 |
| `tokens-blocklist.yaml` | `solana-labs/token-list` long-tail mints | 2,000 |
| **Total** | — | **2,010 rule instances · 8 detector types** |

Packs are also `include_str!`-embedded into the `race-server` binary so production deployments are self-contained — no filesystem dependency.

Regenerate from public data:

```bash
python3 scripts/gen-rule-packs.py --limit 2000
```

### Honest framing

- **8 detector types** is the truthful denominator. The 2,003 blocklist entries are all instances of one detector (`blocked_program`).
- The `tokens-blocklist.yaml` pack is generated deterministically from `solana-labs/token-list`. Anyone can diff the output against the public list.
- The 3 exploit entries are curated placeholders, not pulled from a threat-intel feed — the right next step is plumbing in Webacy / GoPlus / on-chain post-mortem feeds.

## API

Full reference docs in [`docs/api/`](docs/api/):

| Doc | What it covers |
|-----|----------------|
| [`sak-sdk.md`](docs/api/sak-sdk.md) | `Kernel::new`, `submit()`, `with_guardian`, `with_reflex`, `with_state` + `Decision`, `TxMeta`, `ChainEvent` types |
| [`sak-guardian.md`](docs/api/sak-guardian.md) | `Guardian::from_yaml`, `from_yaml_files`, `from_yaml_strings`, `with_rules`, `stats()`, `evaluate`, `evaluate_raw`, all `Rule` variants |
| [`race-server.md`](docs/api/race-server.md) | HTTP/WS demo endpoints — `/evaluate`, `/rules/stats`, `/sol-price`, `/feedback`, `/ws` with request/response JSON |

### Guardian (minimal)

```rust
use sak_guardian::Guardian;
use sak_core::{Decision, TxMeta};

let mut guardian = Guardian::from_yaml_files(&[
    "packs/defaults.yaml",
    "packs/solana-core.yaml",
    "packs/exploits-blocklist.yaml",
    "packs/tokens-blocklist.yaml",
])?;

match guardian.evaluate(&tx, &TxMeta { slippage_bps: Some(9900), ..Default::default() }) {
    Decision::Allow => sign_and_broadcast(tx),
    Decision::Reject { rule, reason } => warn!("Blocked by {rule}: {reason}"),
}

let stats = guardian.stats();
println!("{} rules across {} packs", stats.total, stats.packs.len());
```

### SDK (full stack)

```rust
use sak_sdk::{Kernel, KernelConfig};

let mut kernel = Kernel::new(KernelConfig::default())?
    .with_guardian("rules.yaml")?;

match kernel.submit(&tx, &TxMeta::default()) {
    Decision::Allow => println!("Safe — proceeding"),
    Decision::Reject { rule, reason } => println!("Blocked: {rule} — {reason}"),
}
```

## Demo

Live safety dashboard — Guardian blocks malicious LLM-generated intents in real time. Live slot counter shows the Yellowstone oracle feed. Rule count is read from `GET /rules/stats` so the UI never lies about how many policies are loaded.

```bash
# Terminal 1: race-server (port 3001) — loads all packs from ./packs/
cargo run -p race-server

# Terminal 2: demo UI (port 4000)
cd demo/race-ui && npx vite
```

Dashboard panels: flow diagram (Agent → Guardian → Solana), live execution trace with rule name + reason per tx, transaction log with prevented loss in USD, and live devnet slot counter.

**No API key needed.** Demo Mode runs on scripted attack scenarios but evaluates every intent against the real Rust Guardian with the real loaded rule packs.

## Evil Corpus

24 tests. All pass. Every pattern is blocked by at least one rule:

| # | Attack Pattern | Rule Fired | Severity |
|---|----------------|-----------|----------|
| 1 | 99% slippage swap | `max_slippage` | critical |
| 2 | Wrong token mint (fake USDC) | `allowed_programs` | high |
| 3 | Drain entire SOL balance | `max_account_drain` | critical |
| 4 | Unknown program ID | `allowed_programs` | high |
| 5–20 | Flash loans, compute bombs, CPI loops, priority-fee abuse, dust attacks, account substitution, … | _various_ | low–critical |
| 21 | Swap touching a `blocked_program` | `<pack rule>` | medium |
| 22 | Clean tx against 2,000-rule blocklist | _allowed_ | — |
| 23 | Malicious tx against 2,000-rule blocklist | `real_scam` | medium |
| 24 | `Guardian::stats()` truthfulness | — | — |

## Project Structure

```
crates/
  sak-core/        shared types (Decision, TxMeta, ChainEvent, GuardianFeedback)
  sak-guardian/    LiteSVM simulation + indexed rule evaluation
  sak-reflex/      Yellowstone Geyser gRPC subscriber
  sak-state/       ZK-compressed agent state (stub)
  sak-sdk/         public Kernel API (submit, with_guardian, …)
  sak-bin/         CLI daemon
demo/
  race-server/     Axum HTTP + WS server (evaluate, rules/stats, sol-price, feedback, /ws)
  race-ui/         Standalone HTML dashboard (Vite dev server, port 4000)
  tx-generator/    Generates 70/30 evil/valid transaction stream
packs/             Guardian rule packs (defaults, solana-core, exploits, tokens-blocklist)
scripts/
  gen-rule-packs.py    Regenerates packs from solana-labs/token-list
  bundle-static-demo.sh Builds .pages-out/ for Cloudflare Pages
  deploy-devnet-demo.sh Wrangler deploy wrapper
docs/
  api/             API reference (sak-sdk, sak-guardian, race-server)
  rule-packs/      Documented-but-not-enforced mint lists
```

## Team

Balaji Segu Krishnaiah, Prateek C, Sai Shreyas Gubbi Harish, Tejas Shiv Kumar.

Built for the Colosseum Frontier hackathon — Infrastructure track.

## License

MIT.
