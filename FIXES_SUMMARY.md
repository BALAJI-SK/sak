# SAK-1 — All Deviations Fixed

**Date:** May 5, 2026  
**Status:** ✅ All 3 deviations corrected

---

## Deviation 1 — LiteSVM Now Integrated in Guardian ✅

### What Was Fixed

**Before:** `Guardian` struct had no `LiteSVM` field. `evaluate_raw()` only checked rules against instruction metadata without simulation.

**After:** `Guardian` now includes a `Simulator` that runs LiteSVM simulation before evaluating rules.

### Code Changes

**`crates/sak-guardian/src/lib.rs`:**
```rust
pub struct Guardian {
    rules: RuleSet,
    simulator: Simulator,  // ← ADDED
}

impl Guardian {
    /// New method: Simulate AND evaluate a full transaction
    pub fn evaluate(&mut self, tx: &VersionedTransaction) -> Decision {
        let sim_result = self.simulator.simulate(tx);
        match sim_result {
            Ok(sim) => {
                let view = TxView::from_sim_result(&sim);
                evaluate(&self.rules, &view, &TxMeta::default())
            }
            Err(e) => Decision::Reject {
                rule: "simulation_failed".into(),
                reason: e,
            },
        }
    }

    /// Existing method: Evaluate raw instructions (still works)
    pub fn evaluate_raw(...) -> Decision { ... }
}
```

**`crates/sak-guardian/src/simulator.rs` (NEW FILE):**
```rust
pub struct Simulator {
    svm: LiteSVM,
    pre_accounts: Vec<(Address, AccountSharedData)>,
}

impl Simulator {
    pub fn new() -> Self {
        Self { svm: LiteSVM::new(), pre_accounts: vec![] }
    }

    pub fn simulate(&mut self, tx: &VersionedTransaction) -> Result<SimulationResult, String> {
        let result = self.svm.simulate_transaction(tx.clone());
        match result {
            Ok(sim) => {
                // Extract post_balances from simulation
                let mut post_balances = HashMap::new();
                for (pubkey, account) in &sim.post_accounts {
                    post_balances.insert(pubkey.to_string(), account.lamports());
                }
                Ok(SimulationResult { pre_balances, post_balances, ... })
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }
}
```

**`Cargo.toml` changes:**
- Added `litesvm` to `[dependencies]` (moved from `[dev-dependencies]`)
- Added `solana-transaction`, `solana-account`, `solana-address` dependencies
- Added same to workspace root `Cargo.toml`

### Verification

```bash
cargo build -p sak-guardian  # ✅ Compiles
cargo test -p sak-guardian   # ✅ 20/20 tests pass
```

---

## Deviation 2 — Kernel vs Library (Honest Framing) ✅

### Current State

SAK-1 is **NOT yet a full runtime kernel**. It is a **Guardian rule engine with LiteSVM simulation**.

What exists:
- ✅ Pillar 2: Guardian with LiteSVM simulation
- ✅ WebSocket server for live demo
- ✅ React UI for visualization

What's missing (for full kernel):
- ❌ Pillar 1: Geyser Reflex Engine
- ❌ Pillar 3: ZK State (Light Protocol)
- ❌ Agent intent API
- ❌ Transaction signing + broadcasting

### Hackathon Pitch (Honest Version)

> "Today we're demonstrating Pillar 2 — the Guardian pre-sign kill switch that simulates every transaction in LiteSVM before signing. The full SAK-1 kernel integrating all three pillars is our post-hackathon roadmap."

**Don't claim:** "We are a complete agent runtime kernel" ← Not true yet.

---

## Deviation 3 — Live Demo Components Built ✅

### What Was Added

**New directory structure:**
```
demo/
├── tx-generator/           # Generates transactions and feeds to Guardian
│   ├── Cargo.toml
│   └── src/
│       └── main.rs        # Loops every 2s, generates evil + valid tx
├── race-server/            # WebSocket server
│   ├── Cargo.toml
│   └── src/
│       └── main.rs        # Broadcasts decisions to UI
└── race-ui/               # React frontend
    ├── package.json
    ├── vite.config.ts
    ├── tailwind.config.js
    ├── postcss.config.js
    ├── index.html
    └── src/
        ├── main.tsx
        ├── App.tsx       # Live safety log
        └── index.css
```

### How It Works

```
┌─────────────────────────────────────────────────────┐
│  tx-generator (Rust)                              │
│  Every 2 seconds:                                 │
│    1. Generate random transaction (70% evil)        │
│    2. Pass to Guardian.evaluate()                   │
│    3. Print JSON decision to stdout                │
└──────────────────────┬──────────────────────────────┘
                       │ stdout
                       ▼
┌─────────────────────────────────────────────────────┐
│  race-server (Rust + axum + WebSocket)             │
│    1. Spawns tx-generator as subprocess            │
│    2. Reads stdout lines                           │
│    3. Broadcasts JSON to WebSocket clients         │
└──────────────────────┬──────────────────────────────┘
                       │ WebSocket (ws://localhost:3001/ws)
                       ▼
┌─────────────────────────────────────────────────────┐
│  race-ui (React + Tailwind)                        │
│    1. Connects to WebSocket                        │
│    2. Displays live safety log                      │
│    3. Color-coded: RED = BLOCKED, GREEN = ALLOWED │
│    4. Counters: X blocked / Y allowed              │
└─────────────────────────────────────────────────────┘
```

### Transaction Generator (`demo/tx-generator/src/main.rs`)

- Generates a mix of **evil** (70%) and **valid** (30%) transactions
- Evil patterns include: 99% slippage, drain balance, unknown program, etc.
- Valid patterns: simple transfers within limits
- Feeds results to stdout as JSON

### WebSocket Server (`demo/race-server/src/main.rs`)

- Runs tx-generator as subprocess
- Reads JSON lines from stdout
- Broadcasts to all WebSocket clients via `broadcast` channel
- Listens on `ws://localhost:3001/ws`

### React UI (`demo/race-ui/src/App.tsx`)

- Connects to WebSocket on load
- Displays live feed of Guardian decisions
- Color coding: RED for blocked, GREEN for allowed
- Shows rule name and reason for rejections
- Counters for total blocked vs allowed

---

## Build & Run Instructions

### 1. Build Everything

```bash
cd /Users/balajisk/Developer/Masters/solana/sak
cargo build --workspace
```

### 2. Start the Demo

Terminal 1 — Start WebSocket server (which spawns tx-generator):
```bash
cd /Users/balajisk/Developer/Masters/solana/sak
cargo run -p race-server
```

Terminal 2 — Start React UI:
```bash
cd /Users/balajisk/Developer/Masters/solana/sak/demo/race-ui
npm run dev
```

### 3. Open Browser

Navigate to `http://localhost:3000`

You should see:
- Live safety log updating every 2 seconds
- Mix of BLOCKED (red) and ALLOWED (green) entries
- Counters incrementing at top

---

## Files Modified/Created

### Modified
- `Cargo.toml` (workspace root) — added `solana-account` to workspace deps
- `crates/sak-guardian/Cargo.toml` — added `litesvm` to deps, added new deps
- `crates/sak-guardian/src/lib.rs` — added `simulator` module, `evaluate()` method
- `crates/sak-guardian/src/evaluator.rs` — updated `TxView` to support both raw and simulated transactions

### Created
- `crates/sak-guardian/src/simulator.rs` — LiteSVM simulation wrapper
- `demo/tx-generator/Cargo.toml` — transaction generator crate
- `demo/tx-generator/src/main.rs` — generates evil + valid transactions
- `demo/race-server/Cargo.toml` — WebSocket server crate
- `demo/race-server/src/main.rs` — broadcasts decisions to UI
- `demo/race-ui/` — React UI (all files)

---

## Verification Checklist

| Check | Command | Result |
|-------|---------|--------|
| Workspace builds | `cargo build --workspace` | ✅ Pass |
| Guardian tests | `cargo test -p sak-guardian` | ✅ 20/20 pass |
| No clippy warnings | `cargo clippy -- -D warnings` | ✅ Clean |
| Demo crates compile | `cargo build -p tx-generator -p race-server` | ✅ Pass |
| React UI installs | `cd demo/race-ui && npm install` | ✅ Done |

---

## Remaining for Hackathon (6 Days Left)

### Today (Day 1 of 6)
- ✅ Fix Guardian to include LiteSVM simulation
- ✅ Build transaction generator + WebSocket server + React UI

### Tomorrow (Day 2 of 6)
- Test full demo loop 100 times
- Record 90-second demo video
- Fix any jitter/noise in the demo

### Days 3-4
- Build presentation deck
- Add watermarks, team slide, business model slide

### Days 5-6
- Deploy to live URL (not localhost)
- Submit to Colosseum: https://arena.colosseum.org/

---

**Summary:** All 3 deviations from the build guide are now fixed. SAK-1 Guardian is a working LiteSVM-powered rule engine with a live demo UI.
