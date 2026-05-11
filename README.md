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
cargo test -p sak-guardian  # 20/20 evil corpus tests
```

## What SAK Does

| Pillar | Component | Status | What It Does |
|--------|-----------|--------|--------------|
| **Layer 1** | **Guardian** | ✅ Complete | Simulates every tx in LiteSVM before signing. 7 rules (slippage, whitelist, drain, compute, fee, accounts, min transfer). Zero on-chain cost, < 50ms. |
| **Layer 2** | **Squads Policy** | ✅ Complete | On-chain spending-limit enforced via Squads v4 smart accounts. Even if Guardian is bypassed, the chain rejects over-limit txs. Demo multisig on devnet. |
| **Oracle** | **Reflex Engine** | ✅ Complete | Yellowstone Geyser push oracle — emits `ChainEvent` into an async channel within the same slot. No polling, no RPC overhead. |
| **State** | **ZK Compressed State** | 🔧 Stub | In-memory HashMap. Light Protocol ZK-compression is the next milestone — 100–1000× cheaper than standard accounts. API surface stable. |
| **SDK** | **sak-sdk** | ✅ Complete | `Kernel` struct wraps all pillars. One `submit()` call integrates SAK under any agent framework. |

## Architecture

```
AI Agent (LLM intent)
        │
        ▼
┌───────────────────────┐
│  Guardian (Layer 1)   │  LiteSVM simulation + 7 rules
│  sak-guardian         │  < 50ms · off-chain · zero cost
└──────────┬────────────┘
           │ ALLOW
           ▼
┌───────────────────────┐
│  Squads Policy (L2)   │  Spending limit · on-chain enforced
│  v4 Smart Account     │  Defense in depth
└──────────┬────────────┘
           │ WITHIN LIMIT
           ▼
    Solana Blockchain

Background push oracle (same-slot):
┌───────────────────────┐
│  Reflex Engine        │  Yellowstone Geyser → ChainEvent channel
│  sak-reflex           │  SlotUpdate · AccountChanged · ProgramInvoked
└───────────────────────┘
```

## API

Full reference docs in [`docs/api/`](docs/api/):

| Doc | What it covers |
|-----|----------------|
| [`sak-sdk.md`](docs/api/sak-sdk.md) | `Kernel::new`, `submit()`, `with_guardian`, `with_reflex`, `with_state` + `Decision`, `TxMeta`, `ChainEvent` types |
| [`sak-guardian.md`](docs/api/sak-guardian.md) | `Guardian::from_yaml`, `evaluate`, `evaluate_raw`, all 9 `Rule` variants, `rules.yaml` schema, instruction data parsing |
| [`race-server.md`](docs/api/race-server.md) | HTTP/WS demo endpoints — `/evaluate`, `/sol-price`, `/feedback`, `/ws` with request/response JSON |

### Guardian (minimal)

```rust
use sak_guardian::Guardian;
use sak_core::{Decision, TxMeta};

let mut guardian = Guardian::from_yaml("rules.yaml")?;

match guardian.evaluate(&tx, &TxMeta { slippage_bps: Some(9900), .. })? {
    Decision::Allow => sign_and_broadcast(tx),
    Decision::Reject { rule, reason } => {
        warn!("Blocked by {rule}: {reason}");
    }
}
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

Live safety dashboard — Guardian blocks malicious LLM-generated intents in real time. Squads Policy panel shows devnet spending-limit status. Live slot counter shows Yellowstone oracle feed.

```bash
# Terminal 1: race-server (port 3001)
cargo run -p race-server

# Terminal 2: demo UI (port 4000)
cd demo/race-ui && npx vite
```

Dashboard panels: flow diagram (Agent → Guardian → Squads → Solana), live execution trace with rule name + reason per tx, transaction log with prevented loss in USD, and live devnet slot counter.

**No API key needed.** Demo Mode runs on scripted attack scenarios but evaluates against the real Rust Guardian and calls the real `/squads/create-agent-wallet` endpoint.

## Evil Corpus

20 tests. All pass. Every pattern is blocked by at least one rule:

| # | Attack Pattern | Rule Fired | Severity |
|---|----------------|-----------|----------|
| 1 | 99% slippage swap | `max_slippage` | critical |
| 2 | Wrong token mint (fake USDC) | `allowed_programs` | high |
| 3 | Drain entire SOL balance | `max_account_drain` | critical |
| 4 | Unknown program ID | `allowed_programs` | high |
| 5–20 | Flash loans, compute bombs, CPI loops, priority fee abuse, dust attacks, account substitution, … | _various_ | low–critical |

## Rules (`rules.yaml`)

```yaml
rules:
  - name: max_slippage        type: slippage_check         max_bps: 200
  - name: allowed_programs    type: program_whitelist      programs: [Jupiter v6, Orca, System, SPL Token, ATA, ComputeBudget]
  - name: max_account_drain   type: drain_check            max_lamports: 1000000000
  - name: max_compute_units   type: compute_units_check    max_units: 1400000
  - name: max_priority_fee    type: priority_fee_check     max_microlamports: 1000000
  - name: min_transfer_value  type: min_transfer_lamports  min_lamports: 1
  - name: max_accounts        type: account_count_check    max_count: 20
```

Rules run in order — first rejection short-circuits.

## Project Structure

```
crates/
  sak-core/        shared types (Decision, TxMeta, ChainEvent, GuardianFeedback)
  sak-guardian/    LiteSVM simulation + rule evaluation
  sak-reflex/      Yellowstone Geyser gRPC subscriber
  sak-state/       ZK-compressed agent state (stub)
  sak-sdk/         public Kernel API (submit, with_guardian, …)
  sak-bin/         CLI daemon
demo/
  race-server/     Axum HTTP + WS server (evaluate, sol-price, feedback, /ws)
  race-ui/         Standalone HTML dashboard (Vite dev server, port 4000)
  tx-generator/    Generates 70/30 evil/valid transaction stream
docs/
  api/             API reference (sak-sdk, sak-guardian, race-server)
```

## Team

Balaji Segu Krishnaiah, Prateek C, Sai Shreyas Gubbi Harish, Tejas Shiv Kumar.

Built for the Colosseum Frontier hackathon — Infrastructure track.

## License

MIT.
