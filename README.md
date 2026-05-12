# SAK — Solana Agent Kernel

> The execution kernel for Solana AI agents — pre-sign safety for every transaction (payments, data writes, oracle updates, DAO votes), oracle-grade reflexes, and 1000× cheaper on-chain state in one Rust crate.

[![Integrate in 60 seconds →](https://img.shields.io/badge/Integrate%20in%2060%20seconds%20%E2%86%92-7c3aed)](INTEGRATE.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-f4662c)](https://rustup.rs/)

SAK is a **Rust execution kernel** that plugs under any Solana AI agent framework. Every transaction — whether it moves SOL, writes data to an account, submits an oracle update, or casts a DAO vote — is simulated in LiteSVM before the agent ever signs. If the transaction violates any rule, it is rejected before it reaches the network. Zero on-chain cost, zero irreversible mistakes.

## Quick Start

```bash
git clone https://github.com/BALAJI-SK/sak.git
cd sak
cargo build --workspace
cargo test -p sak-guardian  # 30 tests: 29 evil-corpus + 1 pack-load
```

## What SAK Does

| Pillar | Component | Status | What It Does |
|--------|-----------|--------|--------------|
| **Guardian** | `sak-guardian` | ✅ Complete | Simulates every tx in LiteSVM and evaluates against a **2,010-rule** indexed policy set (9 detector types · 4 rule packs). Zero on-chain cost, <50ms. |
| **Oracle** | `sak-reflex` | ✅ Complete | Yellowstone Geyser push oracle via RPCFast — emits `ChainEvent` into an async channel within the same slot. No polling, no RPC overhead. |
| **State** | `sak-state` | 🔧 Stub | In-memory HashMap. Light Protocol ZK-compression is the next milestone — 100–1000× cheaper than standard accounts. API surface stable. |
| **SDK** | `sak-sdk` | ✅ Complete | `Kernel` struct wraps all pillars. One `submit()` call integrates SAK under any agent framework. |
| **Token data** | `sak-covalent` | ✅ Complete | Covalent GoldRush API — live token verification, wallet balance checks, and wallet risk scoring. Replaces static token data when `COVALENT_API_KEY` is set. |
| **MEV-safe submit** | `sak-jito` | ✅ Complete | Jito Block Engine — SAK-approved transactions submitted as bundles for MEV-protected execution. `JITO_TIP_LAMPORTS` configures tip. |

## Why Prompt-Level Defenses Aren't Enough

Every jailbreak technique documented against deployed Web3 AI agents (Freysa $50k, ElizaOS memory injection, aixbt dashboard exploit, Teeception CTF) manipulates the **LLM's intent layer** — the language the model generates. SAK operates one layer lower: it evaluates **raw transaction bytes in LiteSVM before the agent ever signs**, making it blind to whatever the LLM was convinced to say.

| Real attack vector | What it does to the LLM | What SAK's Guardian sees |
|--------------------|--------------------------|--------------------------|
| **Concept substitution** (Freysa) | Redefines `approveTransfer` as the drain function | System-program transfer > 1 SOL → `drain_check` fires |
| **Memory injection** (ElizaOS cross-platform) | Poisons agent memory with attacker address across Discord → blockchain | Drain amount or unknown program → blocked before sign |
| **Social engineering** ("emergency, transfer all SOL") | Agent convinced a large transfer is urgent/safe | Amount > 1 SOL → `drain_check` fires |
| **External data poisoning** (`SELL_ALL_ASSETS()` in news feed) | Agent convinced to execute malicious swap | Slippage > 200 bps → `slippage_check` fires |
| **Agent smuggling** (malicious sub-agent chain) | Command routed through a less-guarded sub-agent | Final tx still must clear Guardian before sign |
| **MCP / function injection** (`<action=drain>`) | Model calls drain toolcall after malicious context | Resulting tx evaluated against full 2,010-rule set |
| **Many-shot context corruption** | Hundreds of fake examples train model to accept harmful responses | Guardian never sees the chat history — only bytes |

The LLM can be fully jailbroken. If the resulting transaction violates any rule, Guardian rejects it. No system prompt required, no trust placed in the model's refusal behavior.

> This is the architecture recommendation from Anthropic's own 2025 AI security research: *"transaction verification by an independent module"* that sits outside the LLM's context window.

### Where SAK sits vs audit-layer tools

Tools like CertiK AI Auditor review smart contract code before deployment — catching bugs in what the contract *can* do. SAK operates at runtime, evaluating every transaction an AI agent *tries to do* before it signs. A perfectly audited contract can still be drained by a jailbroken agent calling its legitimate functions (Freysa: no contract bug, ~$50k lost). SAK is the layer between the agent and the chain.

> *CertiK audits the contract. SAK guards the agent that calls it.*

### SAK is SlowMist's L4 — independently specified

In their 2026 joint security report, SlowMist and Bitget defined a 5-layer AI agent security framework. **SAK implements L4 verbatim:**

> *"L4 on-chain risk analysis and independent signature mechanisms provide additional security isolation, enabling agents to construct transactions without directly accessing private keys, thereby reducing the systemic risks associated with high-value asset operations."*
> — SlowMist + Bitget, *AI Agent Security Report 2026*

| SlowMist layer | Purpose | SAK |
|---|---|---|
| L1 | Security baseline and dev specs | — |
| L2 | Agent permission boundaries, least privilege | — |
| L3 | Real-time threat awareness at external inputs | — |
| **L4** | **On-chain risk analysis + independent signature isolation** | **SAK Guardian** |
| L5 | Continuous audit and log review | `GET /rules/stats`, feedback endpoint |

The same report rates **prompt injection at 🔴 Extremely High severity** and states: *"Without signature isolation or manual confirmation mechanisms, attackers could even trigger automated transactions using malicious skills."* The Guardian is that isolation — it evaluates transaction bytes, not LLM context.

### BEV / sandwich attacks — $540M in 2026

Blockchain Extractable Value exploits (sandwich attacks, front-running) account for over **$540 million in losses in 2026 alone**. The attack forces a swap to execute at manipulated slippage — typically > 90%. SAK's `slippage_check` rule rejects any transaction where agent-declared slippage exceeds 200 bps, **before the transaction reaches the mempool**. The attack is stopped at the signing step, not after execution.

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
│       · 8   global detectors (incl. session)│
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

### What the Guardian covers

SAK evaluates every transaction type an AI agent can produce — not just token swaps:

| Transaction type | Example | Guardian detects |
|---|---|---|
| **Payment / SOL transfer** | Agent drains wallet to attacker | `drain_check`, `min_transfer_lamports` |
| **Repeated small transfers** | Drip drain — 5% every call, total > safe cap | `session_spend_check` |
| **Token swap** | MEV sandwich forces 95% slippage | `slippage_check` |
| **Data write** | Agent writes to wrong account via malicious program | `program_whitelist`, `blocked_program` |
| **Oracle update** | Poisoned MCP injects unknown oracle program | `program_whitelist` |
| **DAO vote** | Jailbroken agent submits vote via unlisted program | `program_whitelist`, `account_count_check` |
| **NFT / metadata** | Agent invokes fake token program | `blocked_program` |
| **Compute abuse** | Malicious plugin triggers compute bomb | `compute_units_check`, `priority_fee_check` |

### Detector types

| Type | What it checks |
|------|---------------|
| `slippage_check` | Agent-declared slippage cap (bps) |
| `program_whitelist` | Reject if any instruction invokes an unlisted program |
| `blocked_program` | Reject if a specific program id appears in the tx (negative list) |
| `drain_check` | Reject system-program transfers exceeding `max_lamports` per tx |
| `session_spend_check` | Reject once **cumulative** SOL transferred this session exceeds cap — catches drip-drain attacks that evade per-tx limits |
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
| **Total** | — | **2,010 rule instances · 9 detector types** |

Packs are also `include_str!`-embedded into the `race-server` binary so production deployments are self-contained — no filesystem dependency.

Regenerate from public data:

```bash
python3 scripts/gen-rule-packs.py --limit 2000
```

### Honest framing

- **9 detector types** is the truthful denominator. The 2,003 blocklist entries are all instances of one detector (`blocked_program`).
- The `tokens-blocklist.yaml` pack is generated deterministically from `solana-labs/token-list`. Anyone can diff the output against the public list.
- The 3 exploit entries are curated placeholders, not pulled from a threat-intel feed — the right next step is plumbing in Webacy / GoPlus / on-chain post-mortem feeds.

## Covalent + Jito Integration

### Covalent GoldRush (`sak-covalent`)

Set `COVALENT_API_KEY` and the `race-server` activates live token intelligence:

| Endpoint | What it returns |
|----------|----------------|
| `POST /covalent/verify-token` | `{ verified: bool }` — checks name + symbol + decimals + logo |
| `POST /covalent/token-balances` | Live SPL token holdings for any wallet |
| `POST /covalent/wallet-risk` | Risk score 0–100 from tx history patterns |
| `POST /covalent/token-metadata` | Quote rate, decimals, logo for any mint address |

Without `COVALENT_API_KEY`, all endpoints return `{ "configured": false }` — the Guardian still works using its static `tokens-blocklist.yaml` pack.

### Jito Block Engine (`sak-jito`)

SAK-approved transactions can be forwarded to Jito for MEV-protected bundle submission. No API key required — Jito bundles are permissionless.

| Endpoint | What it does |
|----------|-------------|
| `POST /jito/submit-bundle` | Submit base64-encoded tx array as a Jito bundle |
| `GET /jito/status/:bundle_id` | Check bundle landed slot + status |
| `GET /jito/info` | Current tip amount, tip account, block engine URL |

Configure tip: `JITO_TIP_LAMPORTS=10000` (default 0.00001 SOL).

**The pipeline:** Guardian evaluates → `Decision::Allow` → submit via Jito bundle → MEV-protected landing.

## API

Full reference docs in [`docs/api/`](docs/api/):

| Doc | What it covers |
|-----|----------------|
| [`sak-sdk.md`](docs/api/sak-sdk.md) | `Kernel::new`, `submit()`, `with_guardian`, `with_reflex`, `with_state` + `Decision`, `TxMeta`, `ChainEvent` types |
| [`sak-guardian.md`](docs/api/sak-guardian.md) | `Guardian::from_yaml`, `from_yaml_files`, `from_yaml_strings`, `with_rules`, `stats()`, `evaluate`, `evaluate_raw`, all `Rule` variants |
| [`race-server.md`](docs/api/race-server.md) | HTTP/WS demo endpoints — `/evaluate`, `/rules/stats`, `/sol-price`, `/feedback`, `/ws`, `/covalent/*`, `/jito/*` with request/response JSON |

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

29 tests. All pass. Every pattern grounded in a documented real-world attack vector:

| # | Attack Pattern | Source | Rule Fired | Severity |
|---|----------------|--------|-----------|----------|
| 1 | 99% slippage swap | Classic MEV | `max_slippage` | critical |
| 2 | Wrong token mint (fake USDC) | Supply chain | `allowed_programs` | high |
| 3 | Drain entire SOL balance | Direct drain | `max_account_drain` | critical |
| 4 | Unknown program ID | MCP injection | `allowed_programs` | high |
| 5–20 | Flash loans, compute bombs, CPI loops, priority-fee abuse, dust attacks, account substitution, … | _various_ | _various_ | low–critical |
| 21 | Swap touching a `blocked_program` | Blocklist pack | `<pack rule>` | medium |
| 22 | Clean tx against 2,000-rule blocklist | Regression | _allowed_ | — |
| 23 | Malicious tx against 2,000-rule blocklist | Blocklist pack | `real_scam` | medium |
| 24 | `Guardian::stats()` truthfulness | Audit | — | — |
| **25** | **Freysa-style concept substitution** (approveTransfer → drain 9 SOL) | Positive Web3 $50k exploit | `max_account_drain` | critical |
| **26** | **BEV sandwich victim** (9,500 bps slippage forced by MEV bot) | SlowMist/Bitget $540M stat | `max_slippage` | critical |
| **27** | **MCP context pollution** (poisoned server injects unknown program) | SlowMist report 2026 | `allowed_programs` | high |
| **28** | **Agent-chain laundering** (multi-hop through unlisted intermediary) | SlowMist "Agent Smuggling" | `allowed_programs` | high |
| **29** | **Drip drain** — 20 × 0.5 SOL; each tx passes per-tx limit, cumulative 2.5 SOL trips 2 SOL session cap | Novel / this analysis | `session_spend_cap` | critical |

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

Balaji Segu Krishnaiah, Sai Shreyas Gubbi Harish, Tejas Shiv Kumar.

Built for the Colosseum Frontier hackathon — Infrastructure track.

## License

MIT.
