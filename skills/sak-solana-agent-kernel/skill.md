---
name: sak-solana-agent-kernel
description: Pre-sign safety kernel for Solana AI agents. Simulates every transaction in LiteSVM before signing, evaluates against 2,010 safety rules, and blocks malicious intents at zero on-chain cost.
metadata:
  version: 1.0.0
  category: Colosseum Hackathon
  author: SAK Team
---

# SAK — Solana Agent Kernel

The execution safety kernel for Solana AI agents. Every transaction is simulated in LiteSVM before the agent ever signs — eliminating capital waste from failed and malicious transactions at zero on-chain cost.

## What This Skill Does

This skill gives your coding agent the ability to:
- **Pre-sign transaction simulation** — test every Solana transaction in a real VM before signing
- **Safety rule evaluation** — check against 2,010 configurable rules (slippage, drain, compute, program whitelist)
- **Real-time chain awareness** — subscribe to Yellowstone Geyser push streams for same-slot state
- **One-call integration** — `kernel.submit(&tx, &meta)` and you're safe

## Input

When building a Solana AI agent, feed this skill:
- Your agent's transaction generation logic
- Safety thresholds (slippage caps, drain limits, compute ceilings)
- Which programs your agent is allowed to call

## Output

The skill produces:
- ALLOW or REJECT decisions for every transaction
- Rule name and reason for rejections
- Evaluation time (<50ms)
- Integration code for your agent framework

## Quick Integration

### Rust SDK

```rust
// Add to Cargo.toml
sak-sdk = { git = "https://github.com/BALAJI-SK/sak" }

// Use in your agent
use sak_sdk::{Kernel, KernelConfig};
use sak_core::{Decision, TxMeta};

let kernel = Kernel::new(KernelConfig::default())?
    .with_guardian("rules.yaml")?;

match kernel.submit(&tx, &TxMeta { slippage_bps: Some(150), .. }) {
    Decision::Allow => broadcast(tx),
    Decision::Reject { rule, reason } => log::warn!("Blocked by {rule}: {reason}"),
}
```

### HTTP API (Language-Agnostic)

```bash
curl -X POST https://race-server-production-c5c9.up.railway.app/evaluate \
  -H "Content-Type: application/json" \
  -d '{"slippage_bps": 9900, "amount_lamports": 10000000000, "program_ids": ["11111111111111111111111111111111"], "compute_units": 0}'
```

Response:
```json
{
  "decision": "rejected",
  "rule": "max_slippage",
  "reason": "slippage 9900bps exceeds max 200bps",
  "attack_type": "99% Slippage Swap",
  "severity": "critical",
  "simulation_time_ms": 8
}
```

## Architecture

```
AI Agent (LLM intent)
        │
        ▼
┌───────────────────────┐
│  Guardian (Pillar 1)  │  LiteSVM simulation + 2,010 rules
│  sak-guardian         │  < 50ms · off-chain · zero cost
└──────────┬────────────┘
           │ ALLOW
           ▼
     Solana Blockchain

Background (same-slot):
┌───────────────────────┐
│  Reflex Engine        │  Yellowstone Geyser → ChainEvent channel
│  sak-reflex           │  SlotUpdate · AccountChanged · ProgramInvoked
└───────────────────────┘
```

## Safety Rules (2,010 Total)

| Pack | Rules | What It Does |
|------|-------|--------------|
| defaults.yaml | 6 | Slippage cap, drain limit, compute ceiling, priority fee, min transfer, account count |
| solana-core.yaml | 1 | Whitelist of 41 curated mainnet programs |
| exploits-blocklist.yaml | 3 | Curated scam/drainer program IDs |
| tokens-blocklist.yaml | 2,000 | Long-tail SPL mints (blocklist) |

## Evil Corpus — 30 Tests, 30 Passes

Grounded in real Web3 attack vectors (SlowMist, Bitget, Positive Web3):
- 99% slippage swap → blocked by `max_slippage`
- Drain entire SOL balance → blocked by `max_account_drain`
- Unknown program ID → blocked by `allowed_programs`
- Compute bomb (>1.4M units) → blocked by `max_compute_units`
- Freysa-style concept substitution → blocked by `max_account_drain`
- BEV sandwich victim → blocked by `max_slippage`
- MCP context pollution → blocked by `allowed_programs`
- Drip drain (20 × 0.5 SOL) → blocked by `session_spend_check`

All 30 pass: `cargo test -p sak-guardian`

## Live Demo

**No API key needed.** Demo Mode runs on scripted attack scenarios but evaluates against the real Rust Guardian.

- Dashboard: https://sak-devnet-test.pages.dev
- Backend API: https://race-server-production-c5c9.up.railway.app
- Health check: https://race-server-production-c5c9.up.railway.app/health

## Workspace Structure

```
crates/
  sak-core/       Shared types: Decision, TxMeta, ChainEvent
  sak-guardian/   LiteSVM simulation + rule evaluation (2,010 rules)
  sak-reflex/     Yellowstone Geyser gRPC subscriber
  sak-state/      ZK-compressed agent state stub
  sak-sdk/        Public Kernel API
  sak-bin/        CLI daemon
packs/            Guardian rule packs (4 YAML files)
demo/
  race-server/    Axum HTTP + WebSocket server
  race-ui/        Standalone dashboard
docs/api/         API reference
```

## Links

| Resource | URL |
|----------|-----|
| GitHub | https://github.com/BALAJI-SK/sak |
| Live Demo | https://sak-devnet-test.pages.dev |
| Backend API | https://race-server-production-c5c9.up.railway.app |
| API Docs | https://github.com/BALAJI-SK/sak/tree/main/docs/api |

## Team

| Name | Role |
|------|------|
| Balaji Segu Krishnaiah | Rust kernel, Guardian, Reflex Engine, demo |
| Sai Shreyas Gubbi Harish | Demo UI, integration |
| Tejas Shiv Kumar | Testing, documentation |
