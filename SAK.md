# CLAUDE.md — SAK-1 Project Context
> Read this entire file before touching any code, suggesting any architecture, or answering any question about this project.

---

## What This Project Is

**SAK-1 (Solana Agent Kernel)** is a Rust middleware kernel that sits between an LLM-driven agent and the Solana blockchain. It is the execution and safety layer — not the AI, not the blockchain, not the agent framework. Everything in between.

**One-line definition:**
> SAK-1 gives AI agents same-slot reflexes, a pre-sign kill switch, and 1000x cheaper state storage — in one Rust kernel that plugs under any existing agent framework.

**Grandma test:**
> "We give crypto bots a tiny kernel inside Solana so their trades always land perfectly — no wasted money, no stuck transactions."

---

## What SAK-1 Is NOT Building

Never suggest building these. They are explicitly out of scope.

- ❌ A new LLM or AI model
- ❌ A new Geyser plugin (use Yellowstone via Helius/GetBlock/Triton)
- ❌ A new SVM (use LiteSVM — maintained by Anza)
- ❌ A new ZK system (use Light Protocol)
- ❌ EVM support (Solana only, always)
- ❌ A new agent framework (SAK-1 plugs under elizaOS/Agent Kit)
- ❌ A DEX or trading protocol

---

## The Three Problems Being Solved

### Problem 1 — The Polling Tax (Pillar 1)
Agents poll RPC every 400ms asking "did anything change?" By the time the response arrives, the opportunity window is gone. Solana produces a new slot every 400ms. Polling agents are always 2-3 slots behind.

**Data:** Opportunities vanish in under 400ms on Solana. MEV windows are sub-second.

### Problem 2 — The Hallucination Risk (Pillar 2)
LLMs are probability machines. When generating a Solana transaction, the model can hallucinate:
- Wrong token mint address (valid-looking base58, doesn't exist)
- 99% slippage (drains wallet)
- Wrong CPI program layout (fails at execution)
- Incorrect decimal precision (sends wrong amount)
- Unknown programs (unverified code)

Nothing exists between LLM output and transaction signing in any current agent framework.

**Data:** April 2026 — researchers found 26 active malicious LLM routers. One drained $500,000 from a single wallet. ~$1.8B in LLM-linked crypto losses in 2025.

### Problem 3 — The Cost Bottleneck (Pillar 3)
Storing agent state in regular Solana accounts costs ~$0.30 per account. 1,000 agents = $300,000 in rent before they execute a single transaction. This makes micro-agent swarms economically impossible.

**Data:** Light Protocol delivers 100x-1000x cheaper rent via ZK compression. 1,000 agents goes from $300,000 to $300.

---

## The Three Pillars (Solutions)

### Pillar 1 — Reflex Engine
**Crate:** `sak-reflex`
**Tech:** Yellowstone gRPC (Geyser), tokio, tonic, tokio-stream
**What it does:** Subscribes to Geyser push streams instead of polling RPC. Receives account deltas within the same slot they occur. Maintains a lock-free LRU cache of account state with slot-stamped watermarks.

**Key behaviour:**
- Geyser fires account delta → Reflex Engine routes to subscribed agents → agent reacts within same slot
- Reconnect-with-backoff is mandatory — Geyser streams disconnect under load
- Median target lag: under 50ms from event to agent notification

**Providers (all speak same Yellowstone protocol):**
- Helius (recommended for hackathon — free devnet tier, covers Pillar 3 too)
- GetBlock ($400/mo Yellowstone addon)
- Triton (Dragon's Mouth)
- QuickNode

### Pillar 2 — Guardian
**Crate:** `sak-guardian`
**Tech:** LiteSVM, serde, serde_yaml, custom rule evaluator
**What it does:** Every transaction the LLM generates is simulated locally inside LiteSVM before signing. Hard rules defined in YAML are evaluated against the simulation result. If any rule fails, the transaction is rejected with a structured reason. Zero on-chain cost for rejections.

**Rule engine (YAML format):**
```yaml
rules:
  - name: max_slippage
    type: slippage_check
    max_bps: 200          # 2% max

  - name: allowed_programs
    type: program_whitelist
    programs:
      - JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4  # Jupiter v6
      - whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc   # Orca Whirlpool
      - 11111111111111111111111111111111                # System program

  - name: max_tx_value_usd
    type: value_check
    max_usd: 1000

  - name: max_account_drain
    type: drain_check
    max_lamports: 1000000

  - name: decimal_validation
    type: decimals_check
    token: USDC
    expected_decimals: 6
```

**Decision output:**
```rust
pub enum Decision {
    Allow,
    Reject { reason: String, rule: String },
}
```

**Evil corpus (20 minimum malicious tx patterns to test against):**
1. 99% slippage swap
2. Wrong token mint (fake USDC address)
3. Drain entire SOL balance
4. Unknown program ID
5. Transfer to attacker wallet disguised as fee
6. Slippage set to u64::MAX
7. Jupiter route through unlisted pool
8. Zero amount swap (dust attack setup)
9. Account substitution (swap out recipient)
10. Balance underflow after tx
11. Excessive compute units requested
12. Reentrancy-style CPI loop
13. Fake system program ID
14. Multiple drain instructions in one tx
15. Slippage bypassed via CPI
16. Token account closed mid-tx
17. Priority fee set to 100x normal
18. Memo field with injected instruction
19. Tx with 30+ accounts (obfuscation)
20. Simulated tx that passes but mainnet would fail

**LiteSVM known gotchas:**
- Drifts from mainnet on sysvars and recent blockhashes
- Cross-check critical paths with `solana-test-validator` in fork mode
- Compressed account read latency is higher than regular accounts

### Pillar 3 — ZK State
**Crate:** `sak-state`
**Tech:** Light Protocol SDK (light-sdk, light-compressed-token, light-compressed-account), Helius RPC
**What it does:** Agent state (decisions, positions, cooldowns, violation history) stored in ZK-compressed accounts via Light Protocol. Only a 32-byte Merkle root lives on-chain. Full data in Solana ledger. 128-byte ZK proof (Groth16 SNARK) per access.

**How ZK compression works:**
1. Multiple agent states → hashed recursively → single 32-byte root stored on-chain
2. Full data stored off-chain in Solana ledger
3. Transaction includes: off-chain data + Merkle proof + validity ZK proof
4. Validator verifies 128-byte proof against on-chain root
5. Proof size is constant 128 bytes regardless of number of accounts proven

**Critical tradeoff:**
- Compressed account reads have higher latency than regular accounts
- Solution: in-memory cache for hot state (current slot data) + periodic flush to ZK state (cold storage)
- Never read ZK state in the critical path of Pillar 1's reflex loop

---

## System Architecture

```
┌─────────────────────────────────────────┐
│  LLM / Agent Logic (elizaOS, custom)    │  ← NOT our problem
└────────────────┬────────────────────────┘
                 │  Intent: "swap 100 USDC → SOL, max 2% slippage"
┌────────────────▼────────────────────────┐
│  Layer 5: Agent SDK (sak-sdk)           │  ← Public API
│  Rust core + TypeScript bindings        │
├─────────────────────────────────────────┤
│  Layer 4: Guardian (sak-guardian)       │  ← Pillar 2
│  LiteSVM simulation + rule engine       │
├─────────────────────────────────────────┤
│  Layer 3: ZK State (sak-state)          │  ← Pillar 3
│  Light Protocol compressed accounts     │
├─────────────────────────────────────────┤
│  Layer 2: Reflex Engine (sak-reflex)    │  ← Pillar 1
│  Yellowstone Geyser push streams        │
├─────────────────────────────────────────┤
│  Layer 1: Solana RPC + Geyser clients   │
└────────────────┬────────────────────────┘
                 │
            Solana Network
```

**Data flow rules:**
- Events flow UP: Geyser → Reflex Engine → Agent
- Transactions flow DOWN: Agent → Guardian → RPC
- Agent NEVER touches RPC directly
- Every outbound tx passes through Guardian — no exceptions

---

## Repository Structure

```
sak-1/
├── Cargo.toml                  # workspace root
├── CLAUDE.md                   # this file
├── README.md                   # project readme with demo link
├── crates/
│   ├── sak-core/               # shared types, errors, traits
│   ├── sak-reflex/             # Pillar 1: Geyser subscriber + event router
│   ├── sak-guardian/           # Pillar 2: LiteSVM simulator + rule engine
│   ├── sak-state/              # Pillar 3: Light Protocol wrapper
│   ├── sak-sdk/                # Public agent-facing API
│   └── sak-bin/                # CLI / daemon binary
├── bindings/
│   └── sak-ts/                 # napi-rs TypeScript bindings
├── demo/
│   ├── race-ui/                # React dashboard (Vite + Tailwind)
│   ├── race-server/            # WebSocket server (axum + tokio-tungstenite)
│   └── elizaos-control/        # Vanilla polling agent for comparison
├── tests/
│   ├── evil-llm-corpus/        # 20+ malicious tx samples
│   └── integration/
├── infra/
│   ├── docker-compose.yml
│   └── grafana/
├── docs/
│   ├── architecture.md
│   ├── getting-started.md
│   └── rules-spec.md
└── .github/workflows/ci.yml
```

---

## Tech Stack

### Core Runtime
```toml
[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
solana-sdk = "1.18"
solana-client = "1.18"
anchor-lang = "0.30.0"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
```

### Pillar 1 — Reflex Engine
```toml
yellowstone-grpc-client = "*"   # Triton One — Geyser streaming
tonic = "*"                      # gRPC framework
tokio-stream = "*"               # stream combinators
```

### Pillar 2 — Guardian
```toml
litesvm = "0.4"                  # Anza-maintained local SVM
```

### Pillar 3 — ZK State
```toml
light-sdk = "*"
light-compressed-token = "*"
light-compressed-account = "*"
```

### SDK Bindings
```toml
napi-rs = "*"                    # TypeScript bindings
```

### Demo UI
- React + Vite + Tailwind
- axum + tokio-tungstenite (WebSocket server)
- Side-by-side race: SAK-1 agent vs polling agent

### Observability
- tracing + tracing-subscriber (structured logs)
- prometheus crate + Grafana
- Live safety log = structured tracing events piped to UI

---

## Public SDK API (Target Interface)

This is what agent developers will actually call. Design everything to make this interface clean.

```rust
// Rust SDK
let kernel = Kernel::new(config).await?;

// Subscribe to on-chain events (Pillar 1)
kernel.subscribe(filter, |event| async move {
    // react within same slot
}).await?;

// Submit transaction through Guardian (Pillar 2)
match kernel.submit(tx).await? {
    Decision::Allow(signature) => {
        println!("landed: {}", signature);
    }
    Decision::Reject { reason, rule } => {
        println!("blocked by {}: {}", rule, reason);
        // zero on-chain cost
    }
}

// Read/write agent state (Pillar 3)
kernel.state().set("position", &position_data).await?;
let position = kernel.state().get::<Position>("position").await?;
```

```typescript
// TypeScript SDK (napi-rs bindings)
import { Kernel } from '@sak-1/sdk';

const kernel = await Kernel.new(config);

await kernel.subscribe(filter, async (event) => {
  const tx = await agent.generateTransaction(event);
  const result = await kernel.submit(tx);
  
  if (result.type === 'rejected') {
    console.log(`blocked: ${result.reason}`);
  }
});
```

---

## Competitive Positioning

### Where SAK-1 Sits

```
┌─────────────────────────┐
│  LLM (GPT, Claude, etc) │  ← Brain (NOT our layer)
├─────────────────────────┤
│  elizaOS / Agent Kit    │  ← Orchestration (NOT our layer)
├─────────────────────────┤
│  SAK-1 Kernel  ◄────────│  ← WE ARE HERE
├─────────────────────────┤
│  Solana Network         │  ← Settlement (NOT our layer)
└─────────────────────────┘
```

### What We Are Not Competing With
- **elizaOS** — orchestration layer, we plug under it
- **Solana Agent Kit** — action library, we plug under it
- **GetBlock / Helius / Triton** — RPC providers, we USE them
- **Jito** — MEV bundle protection, different layer
- **Light Protocol** — ZK primitive, we USE it
- **LiteSVM** — simulation engine, we USE it

### The One True Competitor Gap
No project in Colosseum history has composed Geyser push + LiteSVM simulation + ZK state into a single agent runtime kernel. That composition is the product.

---

## Why Large Context Windows Don't Replace SAK-1

This objection will come up. The answer:

1. **Context ≠ accuracy** — LLMs predict probable tokens, not correct on-chain state
2. **Inference is 2-10 seconds** — Solana slots are 400ms — you're always stale
3. **Rules need enforcement, not memory** — "don't exceed 2% slippage" in context is a request. Guardian is a hard constraint
4. **Novel hallucinations have no prior pattern** — a fake mint address looks statistically normal
5. **Cost** — 1M token context at $15/M tokens × 1000 tx/day = $15,000/day. Small LLM + SAK-1 = $0.40/day

---

## Why Normal Fraud Detection Doesn't Work

1. **No reversal on blockchain** — fraud detection is reactive, blockchain is final
2. **No historical baseline** — agents are autonomous, every transaction is novel
3. **Speed mismatch** — fraud detection takes seconds, Solana slots are 400ms
4. **Can't read execution intent** — fraud detection sees metadata, Guardian simulates full SVM execution
5. **Statistical vs deterministic** — fraud detection gives probability, Guardian gives guarantee

---

## Demo Requirements (For Hackathon)

The demo must show one thing working end-to-end. Priority order:

### MVP Demo (Guardian only — buildable in 3 days)
1. LLM generates malicious transaction (99% slippage swap)
2. Guardian simulates locally via LiteSVM
3. Rule engine evaluates: slippage_check fails
4. Rejection returned with reason + rule name
5. React UI shows live safety log of blocked transactions
6. Zero on-chain cost demonstrated

### Full Demo (all three pillars)
Two-column React UI:
- Left: SAK-1 agent reacting via Geyser push
- Right: Vanilla polling agent (elizaOS-style)
- Same trigger event drives both
- Latency counter showing slot delta
- Live safety log showing Guardian rejections

**Demo recording rules:**
- Run race 100 times to filter devnet jitter
- Use representative runs, not cherry-picked best
- Record exactly 90 seconds
- Show the working product in first 10 seconds

---

## Known Failure Modes To Handle

| Failure | Mitigation |
|---|---|
| Geyser stream disconnects | Reconnect-with-backoff mandatory from day 1 |
| LiteSVM drifts from mainnet on sysvars | Cross-check with fork mode |
| Compressed account read latency interferes with reflex loop | In-memory cache + periodic flush |
| Devnet jitter makes latency comparison noisy | Run race 100 times |
| Scope creep to EVM/custom LLM/custom SVM | Reject all until v2 |

---

## Build Priority Order

Given 6 days to submission, build in this order:

**Day 1:** Cargo workspace, LiteSVM hello world, one tx simulated
**Day 2:** Guardian core — 20 evil corpus tests all passing
**Day 3:** Structured rejection log piped to React UI
**Day 4:** Record 90-second demo video
**Day 5:** Fix deck (watermarks, team slide, business model slide)
**Day 6:** Submit + post on X

If time allows, add Pillar 1 (Geyser) after Guardian is solid. Pillar 3 (ZK State) is lowest priority for the hackathon demo.

---

## Environment Setup

```bash
# Required accounts/keys
HELIUS_API_KEY=           # Get from helius.dev — free tier covers devnet
SOLANA_RPC_URL=           # https://devnet.helius-rpc.com/?api-key=YOUR_KEY

# Recommended infrastructure
# Hetzner AX server (Frankfurt or Amsterdam) — low latency to Solana validators
# Avoid AWS for latency-sensitive Geyser work

# CI requirements
cargo test                # all tests pass
cargo clippy -- -D warnings   # zero warnings
cargo fmt --check         # formatted
```

---

## Code Style Rules

- Use `anyhow::Result` for application errors, `thiserror` for library errors
- All async code uses `tokio`
- Structured logging via `tracing` — never use `println!` in library code
- Every public function has a doc comment
- Every Guardian rule has a corresponding evil corpus test
- Reconnect logic uses exponential backoff with jitter — never fixed sleep

---

## Business Model

```
Free tier:    5,000 intents/month
Starter:      $99/month → 100,000 intents
Growth:       $499/month → 1,000,000 intents
Enterprise:   $0.015/intent, VPC deployment, SLA

Target: 250 dev teams → $75k MRR by month 6
Breakeven: 9.8M intents/month
```

---

## Hackathon Context

**Event:** Colosseum Frontier
**Deadline:** May 11, 2026
**Prize:** $30K Grand Champion + $10K University Team prize (team are all MSc students)
**Judging criteria:** Most impactful project for Solana ecosystem

**Team:**
- Balaji Segu Krishnaiah — Founder, MSc AI (DCU), ex-McKinsey, ex-Tejas Networks
- Prateek C — CTO, MSc AI, ex-Tejas Networks
- Sai Shreyas Gubbi Harish — Co-founder, MSc Cloud (NCI), ex-EY
- Tejas Shiv Kumar — CMO, MSc DA (DCU), ex-AceMicromatic

**Submission requirements:**
- Live URL that works (not cloned and run locally by judges)
- GitHub repo with clean README: problem in one line, demo URL in line two, GIF in line three
- 90-second demo video embedded on landing page
- Custom domain (not a GitHub pages URL)

---

## Key Marketing Messages (For Any Content)

**To technical builders:**
> "One SDK import. Every LLM-generated transaction simulated and validated before signing. Same-slot reflexes via Geyser. 1000x cheaper state via ZK compression."

**To non-technical founders:**
> "We stop AI bots from making expensive mistakes with your money — automatically, before it happens."

**To VCs/judges:**
> "Every agent framework tells you what to build. SAK-1 is the first runtime that makes it safe to ship."

**The one-liner:**
> "Ship agents that can't be used against you."

---

## References

- Solana docs: https://solana.com/docs
- Yellowstone gRPC: https://github.com/rpcpool/yellowstone-grpc
- LiteSVM: https://github.com/LiteSVM/litesvm
- Light Protocol: https://www.lightprotocol.com/
- ZK Compression docs: https://www.zkcompression.com/
- Helius docs: https://docs.helius.dev/
- Anchor: https://www.anchor-lang.com/
- napi-rs: https://napi.rs/
- Colosseum Frontier: https://arena.colosseum.org/

---

*Last updated: May 2026 | Status: Active hackathon build*
