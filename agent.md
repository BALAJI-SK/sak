# SAK — Agent Context Document

> Full codebase reference for AI agents and developers. Read this before touching any file.

---

## What SAK Is

SAK (Solana Agent Kernel) is a **Rust middleware kernel** that sits between an LLM-driven agent and the Solana blockchain. It is the execution and safety layer — not the AI, not the blockchain.

**Core value props:**
- Pre-sign simulation of every transaction (no on-chain cost)
- Same-slot reflex via Yellowstone Geyser push streams
- 100–1000× cheaper agent state via ZK-compressed accounts

**Hackathon:** Colosseum Frontier — deadline May 11, 2026

---

## Workspace Layout

```
sak/
├── Cargo.toml                  # workspace root (resolver = "2")
├── index.html                  # static Guardian + NVIDIA gate demo (mirrored in demo/race-ui/)
├── agent.md                    # this file — agent / maintainer context
├── rules.yaml                  # Guardian rule definitions (read at runtime)
├── .env                        # gitignored — real secrets go here
├── .env.example                # committed placeholder values
├── scripts/
│   ├── bundle-static-demo.sh   # → .pages-out/ for Cloudflare Pages
│   └── deploy-devnet-demo.sh   # bundle + wrangler deploy → sak-devnet-test
├── .github/workflows/
│   └── deploy-github-pages.yml # optional: GitHub Actions → GitHub Pages
│
├── crates/
│   ├── sak-core/               # Shared types (ChainEvent, Decision, TxMeta, …)
│   ├── sak-guardian/           # Pillar 2 — LiteSVM pre-sign simulation + rules
│   ├── sak-reflex/             # Pillar 1 — Yellowstone gRPC slot subscriber
│   ├── sak-state/              # Pillar 3 — ZK-compressed agent state (stub)
│   ├── sak-sdk/                # Public API: Kernel struct wraps all 3 pillars
│   └── sak-bin/                # CLI daemon — runs Guardian + Reflex Engine
│
└── demo/
    ├── race-server/            # Axum WebSocket server (port 3001)
    ├── race-ui/                # Standalone HTML demo (served on port 4000)
    └── tx-generator/           # Generates evil/valid transactions (70/30 mix)
```

---

## Workspace Dependencies

Defined in root `Cargo.toml` `[workspace.dependencies]`. Key pins:

| Crate | Version | Notes |
|---|---|---|
| `tokio` | 1, features=["full"] | async runtime |
| `litesvm` | 0.11 | local Solana VM for simulation |
| `yellowstone-grpc-client` | 5 (locked 5.1.0) | Geyser gRPC |
| `tonic` | 0.12 | gRPC transport |
| `solana-*` | 3.x (modular) | use modular crates, NOT `solana-sdk` |
| `light-*` | `*` | ZK compression (pulls solana-sdk ^2.2) |
| `anyhow` | 1 | error handling |
| `tracing` | 0.1 | structured logging |

**Critical version conflict to know about:**  
`yellowstone-grpc-proto`'s `convert` feature requires `solana-sdk ~2.1.1`, but `light-compressed-token` requires `solana-sdk ^2.2`. These cannot coexist. Solution: always add `yellowstone-grpc-proto` with `default-features = false, features = ["tonic"]` to skip the `convert` feature and its solana-sdk dep.

---

## crates/sak-core

**Path:** `crates/sak-core/src/`  
**Purpose:** Shared types used across all crates. No logic.

### Types (`types.rs`)

```rust
// ChainEvent — enum (was a struct, converted to enum during Pillar 1 implementation)
pub enum ChainEvent {
    AccountChanged { slot: u64, pubkey: String, lamports: u64 },
    ProgramInvoked { slot: u64, program_id: String },
    SlotUpdate { slot: u64, parent: Option<u64>, status: i32 },  // new — Reflex Engine
    ShredEntry { slot: u64, data: Vec<u8> },                     // new — Reflex Engine
}

// EventKind — legacy enum kept for backward compat (mirrors ChainEvent variants minus slot)
pub enum EventKind { AccountChanged, ProgramInvoked, SlotUpdate, ShredEntry }

// Guardian verdict
pub enum Decision { Allow, Reject { rule: String, reason: String } }

// Agent-declared intent alongside a transaction
pub struct TxMeta { slippage_bps: Option<u64>, description: Option<String> }

// Feedback
pub enum FeedbackVerdict { Correct, Wrong, Neutral }
pub struct GuardianFeedback { timestamp, decision, rule, description, stars: u8, verdict }
```

**Exports from `lib.rs`:**
```rust
pub use types::{ChainEvent, Decision, EventKind, TxMeta, GuardianFeedback, FeedbackVerdict};
pub use error::SakError;
```

---

## crates/sak-guardian

**Purpose:** Pillar 2 — simulate every transaction in LiteSVM, evaluate against rules. Zero on-chain cost.

### Public API (`lib.rs`)

```rust
let mut guardian = Guardian::from_yaml("rules.yaml")?;         // load rules from YAML
let mut guardian = Guardian::with_rules(vec![Rule::...])?;     // for tests
let decision = guardian.evaluate(&versioned_tx, &tx_meta);     // simulate + evaluate
let decision = guardian.evaluate_raw(account_keys, ixs, meta); // without simulation
```

### Rule Types (`rules.rs`)

`Rule` is a serde-tagged enum with `#[serde(tag = "type")]`. YAML key `type:` selects the variant.

| Variant | YAML type | Key param |
|---|---|---|
| `SlippageCheck` | `slippage_check` | `max_bps: u64` |
| `ProgramWhitelist` | `program_whitelist` | `programs: Vec<String>` |
| `DrainCheck` | `drain_check` | `max_lamports: u64` |
| `AccountCountCheck` | `account_count_check` | `max_count: usize` |
| `ComputeUnitsCheck` | `compute_units_check` | `max_units: u32` |
| `PriorityFeeCheck` | `priority_fee_check` | `max_microlamports: u64` |
| `MinTransferLamports` | `min_transfer_lamports` | `min_lamports: u64` |
| `ValueCheck` (stub) | `value_check` | always passes |
| `DecimalsCheck` (stub) | `decimals_check` | always passes |

### Evaluator (`evaluator.rs`)

- `TxView` enum: `Raw` (for `evaluate_raw`) or `Simulated` (for `evaluate` — uses LiteSVM balances)
- Drain detection on `Simulated` path uses actual pre/post balance diff from LiteSVM
- Drain detection on `Raw` path parses system program bincode instruction data:
  - Transfer = discriminant `2`, then `u64 LE` lamports at bytes 4–11
  - ComputeBudget uses `0x02` (SetComputeUnitLimit) and `0x03` (SetComputeUnitPrice) with 1-byte discriminants
- Rules are evaluated in order; first failure short-circuits

### Simulator (`simulator.rs`)

- Wraps `litesvm::LiteSVM`
- Only supports `VersionedMessage::Legacy` (V0 messages return an error)
- Snapshots pre-balances from `svm.get_account()` before simulation
- Post-balances come from `sim.post_accounts`

### Tests (`tests/evil_corpus.rs`)

20 integration tests, all must assert `Decision::Reject`. Uses real `LiteSVM`, `solana-keypair`, actual transactions. Run with:
```bash
cargo test -p sak-guardian
```
Expected: `test result: ok. 20 passed; 0 failed`.

---

## crates/sak-reflex

**Purpose:** Pillar 1 — subscribe to Yellowstone Geyser gRPC push stream, emit `ChainEvent` into a channel.

### Config (`config.rs`)

```rust
pub struct ReflexConfig { pub endpoint: String, pub token: String }

impl ReflexConfig {
    pub fn from_env() -> Self  // reads YELLOWSTONE_ENDPOINT + YELLOWSTONE_TOKEN
    pub fn devnet() -> Self    // hardcodes devnet endpoint, reads token from env
    pub fn custom(endpoint, token) -> Self
}
```

Devnet endpoint: `https://sol-devnet-yellowstone-grpc.rpcfast.com:443`  
Token env var: `YELLOWSTONE_TOKEN`

### Top-level `start` function (`lib.rs`)

The primary entry point for Pillar 1:

```rust
pub async fn start(config: ReflexConfig, tx: mpsc::Sender<ChainEvent>) -> Result<()>
```

- Connects with `GeyserGrpcClient::build_from_shared(endpoint)?.x_token(token)?.connect().await?`
- Subscribes to slot updates: `CommitmentLevel::Processed`, filter `SubscribeRequestFilterSlots { filter_by_commitment: Some(true), ..Default::default() }`
- Sends `ChainEvent::SlotUpdate { slot, parent, status }` into the mpsc channel
- Logs `"SAK Reflex Engine connected"` on success
- On any stream error or channel close: breaks inner loop, sleeps 500ms, reconnects

### `SubscribeRequestFilterSlots` gotcha

The struct has an `interslot_updates` field not mentioned in docs. Always use:
```rust
SubscribeRequestFilterSlots {
    filter_by_commitment: Some(true),
    ..Default::default()   // required — struct is non-exhaustive
}
```

### Legacy broadcast-based API (`lib.rs` + `subscriber.rs` + `router.rs`)

Still present for use by `sak-sdk::Kernel`:
- `ReflexEngine::new(endpoint, x_token)` → `(ReflexEngine, EventRouter)`
- `ReflexEngine::run(filter)` — runs the subscriber
- `EventRouter::subscribe(filter_fn, handler)` — callback-based routing
- `GeyserSubscriber` — wraps `GeyserGrpcClient`, same reconnect logic

### Cargo.toml deps of note

```toml
yellowstone-grpc-client = { workspace = true }
yellowstone-grpc-proto = { version = "5", default-features = false, features = ["tonic"] }
# default-features = false is critical — avoids solana-sdk ~2.1.1 conflict
tonic = { workspace = true, features = ["transport", "tls", "tls-native-roots"] }
# tls-native-roots is REQUIRED — without it, ClientTlsConfig::with_native_roots() is a no-op
# and rustls has no CA roots → TLS handshake fails silently with "transport error"
futures = "0.3"   # for StreamExt
```

### TLS native roots gotcha (critical)

`GeyserGrpcClient` uses rustls via tonic. `ClientTlsConfig::new().with_native_roots()` only loads native CA certs when the `tls-native-roots` Cargo feature is active. Without it the flag is silently ignored — rustls has no root certs → TLS handshake fails with `"gRPC transport error: transport error"`.

**Fix:** both `tls` AND `tls-native-roots` features must be listed in `tonic = { features = [...] }`, AND the builder chain must call `.tls_config(ClientTlsConfig::new().with_native_roots())?` explicitly:

```rust
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};

let mut builder = GeyserGrpcClient::build_from_shared(config.endpoint.clone())?
    .tls_config(ClientTlsConfig::new().with_native_roots())?;
```

This is already done in both `sak-reflex/src/lib.rs` (`start()`) and `sak-reflex/src/subscriber.rs` (`connect_and_stream()`).

---

## crates/sak-state

**Purpose:** Pillar 3 — ZK-compressed agent state via Light Protocol. Currently a stub backed by in-memory HashMap.

### API

```rust
let mut zk = ZkState::new();
zk.set("agent-id", &agent_state)?;          // write (in-memory)
let state = zk.get("agent-id")?;            // read (in-memory)
zk.flush_to_zk()?;                          // batch flush to Light Protocol (stub)
```

### `AgentState` schema (`schema.rs`)

```rust
pub struct AgentState {
    pub agent_id: String,
    pub last_n_decisions: Vec<DecisionRecord>,   // {slot, decision, rule}
    pub open_positions: Vec<Position>,            // {token_mint, amount, entry_slot}
    pub cooldown_until_slot: u64,
    pub violation_history: Vec<String>,
}
```

**Status:** Backed by `HashMap<String, AgentState>` in-memory. `flush_to_zk()` logs but does nothing. Light Protocol integration not wired.

---

## crates/sak-sdk

**Purpose:** Public API for agent developers. `Kernel` wraps all 3 pillars.

```rust
let kernel = Kernel::new(config)?
    .with_guardian("rules.yaml")?;

// Submit a transaction
let decision = kernel.submit(&versioned_tx, &tx_meta);

// With reflex engine
let (kernel, mut router) = kernel.with_reflex(endpoint, x_token);
router.subscribe_all(|event| async { /* handle */ }).await;

// With state
let kernel = kernel.with_state()?;
kernel.state().unwrap().set("id", &agent_state)?;
```

`KernelConfig` fields: `geyser_endpoint`, `helius_api_key`, `rules_path`.

---

## crates/sak-bin

**Purpose:** CLI daemon — runs Guardian + spawns Reflex Engine as a non-blocking task.

### main.rs behaviour

1. Loads `rules.yaml` into Guardian
2. Creates `(tx, rx)` mpsc channel (capacity 256)
3. Spawns `sak_reflex::start(ReflexConfig::devnet(), tx)` as a tokio task
4. Spawns a second task: reads from `rx`, logs `"SLOT {slot} — Reflex Engine live"` for `ChainEvent::SlotUpdate`
5. Parks main task with `sleep(u64::MAX)`

**Does not block Guardian pipeline** — Reflex Engine runs independently.

**Deps added this session:** `sak-reflex = { path = "../sak-reflex" }`

---

## demo/race-server

**Port:** 3001  
**Hosted:** Often deployed to **Railway** as `race-server` (HTTPS edge → container `:8080`). See **Hosted demo, Cloudflare & Railway** for env vars, CORS, and typical startup logs.

**Framework:** Axum 0.7 + tokio-tungstenite  
**Deps (relevant):** `sak-guardian`, `sak-reflex`, `sak-core`

### Endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/ws` | WebSocket — streams JSON: tx-generator events AND Yellowstone slot_update events |
| GET | `/sol-price` | Returns `{"usd": <f64>}`. Proxies CoinGecko with 60s server-side cache. |
| POST | `/feedback` | Accepts `GuardianFeedback` JSON, stores in memory |
| GET | `/feedback/summary` | Returns `{total, correct, wrong, accuracy}` |
| POST | `/evaluate` | **Real Rust Guardian evaluation** — takes intent JSON, runs `sak-guardian::evaluate_raw`, returns decision |
| POST | `/squads/create-agent-wallet` | **Squads Layer 2 demo** — returns mock smart account PDA, spending limit, Solscan/Squads URLs, SDK snippet. See below. |

### WebSocket message types on `/ws`

Two distinct message shapes are broadcast on the same channel:

```json
// tx-generator events — NO "type" field; UI handles as tx log entry
{"id":"…","timestamp":…,"decision":"rejected","rule":"max_slippage",…}

// Yellowstone Reflex Engine — has "type" field; UI handles as slot counter
{"type":"slot_update","slot":312345678}
```

**UI routing rule:** `if (parsed.type === 'slot_update')` → update slot counter; else → handle as tx event.

### Yellowstone Reflex Engine in race-server

Spawned at startup in `main()`. Guarded by `YELLOWSTONE_TOKEN` env var:

```rust
let token = std::env::var("YELLOWSTONE_TOKEN").unwrap_or_default();
if token.is_empty() {
    tracing::warn!("YELLOWSTONE_TOKEN not set — Yellowstone Reflex Engine disabled");
} else {
    let config = ReflexConfig::from_env();
    let (chain_tx, mut chain_rx) = tokio::sync::mpsc::channel::<ChainEvent>(256);
    let ws_tx = tx.clone();  // tx = broadcast::Sender<String> shared with /ws handler

    tokio::spawn(async move {
        if let Err(e) = sak_reflex::start(config, chain_tx).await {
            tracing::error!("Reflex Engine fatal: {}", e);
        }
    });

    tokio::spawn(async move {
        while let Some(event) = chain_rx.recv().await {
            if let ChainEvent::SlotUpdate { slot, .. } = event {
                let msg = serde_json::json!({ "type": "slot_update", "slot": slot }).to_string();
                let _ = ws_tx.send(msg);
            }
        }
        tracing::warn!("Reflex Engine channel closed");
    });
}
```

If `YELLOWSTONE_TOKEN` is missing: server starts normally, `/evaluate` and all other endpoints work, the slot counter in the UI just stays `—`.

### POST /evaluate — the real Guardian call

This is how the demo UI calls actual Rust code instead of duplicating logic in JavaScript.

**Request body:**
```json
{
  "slippage_bps":    9900,
  "amount_lamports": 100000000,
  "program_ids":     ["JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"],
  "compute_units":   0,
  "description":     "maximize USDC to SOL swap output"
}
```

**Response:**
```json
{
  "decision":           "rejected",
  "rule":               "max_slippage",
  "reason":             "slippage 9900bps exceeds max 200bps",
  "attack_type":        "99% Slippage Swap",
  "severity":           "critical",
  "simulation_time_ms": 19
}
```

**How intent fields map to raw instructions for `evaluate_raw`:**

The handler builds `account_keys: Vec<String>` and `owned_data: Vec<Vec<u8>>` from the intent:
- Each `program_id` becomes an entry in `account_keys` (index 0 = dummy payer, index 1+ = programs)
- If program = System Program AND `amount_lamports > 0`: encodes a system Transfer instruction (`[0x02,0x00,0x00,0x00, lamports_le_u64]`) so the `DrainCheck` rule can parse it
- If program = ComputeBudget AND `compute_units > 0`: encodes SetComputeUnitLimit (`[0x02, units_le_u32]`)
- All other programs: empty `&[]` data (enough for whitelist check to fire)
- **Auto-inject:** if `compute_units > 0` but ComputeBudget is NOT in `program_ids`, a ComputeBudget instruction is appended so `ComputeUnitsCheck` can still fire

**Guardian rules hardcoded in `default_guardian()`** (same thresholds as `rules.yaml`):

```rust
Rule::SlippageCheck        { max_bps: 200 }
Rule::ProgramWhitelist     { programs: [JUP6, JUP4, Orca, Raydium, SPL Token, ATA, System, ComputeBudget, SysvarRent] }
Rule::DrainCheck           { max_lamports: 1_000_000_000 }   // 1 SOL
Rule::ComputeUnitsCheck    { max_units: 1_400_000 }
Rule::PriorityFeeCheck     { max_microlamports: 1_000_000 }
Rule::MinTransferLamports  { min_lamports: 1 }
```

Rules run in the order listed — first match short-circuits. Implication: if slippage_bps > 200, slippage fires before whitelist even if program is also unknown.

**Attack prompts must use slippage ≤ 200 when testing whitelist violations** — otherwise slippage fires first.

### Logging

`tracing_subscriber::fmt()` initialised with `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))` — defaults to `INFO` if `RUST_LOG` is not set.

Every `/evaluate` call produces three log lines:
```
INFO  race_server:           sak-guardian evaluate_raw called  slippage_bps=… amount_lamports=… compute_units=… programs=[…] desc=…
WARN  sak_guardian::evaluator: Guardian blocked transaction  rule=… reason=…    ← from inside the Rust crate itself
INFO  race_server:           Guardian → REJECT  elapsed_ms=19 rule=… reason=… attack_type=… severity=…
```
(For allowed transactions, only the first and a `Guardian → ALLOW` line appear.)

### SOL Price Cache (`sol-price` endpoint)

- `PriceCache` struct: holds `price: f64` (default 150.0) + `fetched_at: Option<Instant>`
- Stale if not fetched or fetched > 60 seconds ago
- Fetches from CoinGecko server-side (no CORS), 5-second timeout
- Falls back to last known price on error

**Why proxied:** CoinGecko does not set CORS headers for `localhost` origins. Also, calling it on every agent loop iteration causes 429 rate limits. The proxy + 60s cache solves both.

### CORS

`CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)` — allows all origins. Required because the demo HTML is served on port 4000 but calls the server on port 3001.

### Transaction Generator subprocess

Spawned as subprocess: `cargo run --manifest-path demo/tx-generator/Cargo.toml`. Its stdout is piped into the broadcast channel. Restarts automatically on exit with 5-second delay.

**Broken pipe fix:** `tx-generator` uses `writeln!(io::stdout(), …)` + checks the `Result` — if the parent closes the pipe, the loop `break`s cleanly. Old `println!` caused a panic (`failed printing to stdout: Broken pipe`).

### AppState

```rust
struct AppState {
    feedback: Arc<Mutex<Vec<GuardianFeedback>>>,
    price: Arc<Mutex<PriceCache>>,
}
// Note: evaluate_handler creates a fresh Guardian per request (no state needed).
// Guardian::with_rules() creates a dormant LiteSVM; evaluate_raw() never calls simulate().
```

---

## demo/race-ui

**File:** `demo/race-ui/index.html` — standalone HTML file with inline CSS + JS. NOT a Vite/React app. Kept aligned with repo-root **`index.html`** (same gate, `API_BASE`, Stop demo, landing links). Production static demo is deployed from the **root** `index.html` bundle, not this folder.

**Served on:** port 4000 (static file server, e.g. `python3 -m http.server 4000`) or **`bun run dev`** in `demo/race-ui` (Vite proxies to `race-server`). Hosted URLs and `API_BASE` rules: see **Hosted demo, Cloudflare & Railway**.

**Connects to:**
- `http://localhost:3001` — REST: price, evaluate, feedback
- `ws://localhost:3001/ws` — WebSocket: live slot counter (Yellowstone) + tx-generator events (not used for evaluation, just presence)

### Guardian evaluation flow

```
NVIDIA NIM API → intent JSON → evaluateWithBackend(intent)
                                    │
                                    ├─ POST http://localhost:3001/evaluate
                                    │       ↓
                                    │  Rust sak-guardian::evaluate_raw()
                                    │       ↓
                                    │  { decision, rule, reason, attack_type, severity, ms }
                                    │
                                    └─ on fetch error → JS evaluateIntent() fallback
```

**`evaluateWithBackend(intent)`** — the primary evaluation path:
- POSTs intent to `/evaluate`, maps response to entry format
- Adds `via_backend: true` to the entry — the log card renders a purple **⚙ Rust** badge
- If server is down: calls `evaluateIntent(intent)` silently, log card shows orange **JS fallback** badge
- `simulated_loss_usd` is computed in JS: `(intent.amount_lamports / 1e9) * solPrice` (backend doesn't know live SOL price)
- `guardOn === false` path: backend decision is still fetched, but the entry is flipped to `allowed` (threat logged, not blocked)

**`evaluateIntent(intent)`** — JS fallback (runs when race-server is offline):
- Checks same rules: slippage > 200 bps, lamports > 1 SOL, unknown program_ids, compute_units > 1,400,000
- Whitelist: same 10 program IDs as the Rust `default_guardian()`
- Loss USD computed from real `solPrice` variable

### Live slot counter

`connectSlotWS()` IIFE runs immediately on page load (before the gate screen is dismissed):

```javascript
(function connectSlotWS() {
  const dot = document.getElementById('slotDot');
  const counter = document.getElementById('slotCounter');
  let ws;

  function connect() {
    try { ws = new WebSocket('ws://localhost:3001/ws'); } catch (_) { return; }

    ws.onmessage = function(e) {
      let parsed;
      try { parsed = JSON.parse(e.data); } catch (_) { return; }
      if (parsed.type === 'slot_update' && parsed.slot) {
        counter.textContent = Number(parsed.slot).toLocaleString();
        dot.style.opacity = '1';
        dot.classList.add('sak-dot--pulse');
        setTimeout(() => { dot.classList.remove('sak-dot--pulse'); dot.style.opacity = '0.6'; }, 300);
      }
      // non-slot_update messages are silently ignored here (tx-generator events
      // are NOT surfaced through this WS handler — the UI builds entries from
      // the NVIDIA NIM agent loop, not from the tx-generator subprocess)
    };

    ws.onclose = function() { dot.style.opacity = '0.2'; setTimeout(connect, 3000); };
    ws.onerror = function() { ws.close(); };
  }

  connect();
})();
```

The slot counter lives in the footer bar alongside "Guardian Accuracy", "Decisions", and "False Positives". It shows `—` until the first slot arrives. The dot pulses purple on each tick.

**Requires:** `YELLOWSTONE_TOKEN` set in race-server's env. Without it, the WebSocket still connects (no crash), but only tx-generator events arrive — the counter stays `—`.

### Key JS functions

- `fetchSolPrice()` — calls `http://localhost:3001/sol-price` (proxied, not CoinGecko directly)
- `agentLoop()` / `_agentLoopBody()` — every 8 seconds: fetches SOL price → calls NVIDIA NIM → `evaluateWithBackend()` → updates UI
- `callNVIDIA(prompt)` — calls NVIDIA NIM API (`minimaxai/minimax-m2.7` model) via Vite proxy at `/api/nvidia`
- `agentRunning` guard — prevents concurrent `agentLoop` invocations; interval cleared during rate-limit backoff
- `connectSlotWS()` — IIFE, connects to `ws://localhost:3001/ws`, routes `slot_update` messages to footer counter

### NVIDIA attack prompts

Attack/valid alternates on `txCount % 2`. The four attack variants cycle on `txCount % 8`:

| Variant | Key field values | Rule that fires |
|---|---|---|
| Slippage | `slippage_bps: 9900` | `max_slippage` |
| Drain | `amount_lamports: 9950000000`, System Program | `max_account_drain` |
| Unwhitelisted | `slippage_bps: 150`, unknown program | `allowed_programs` |
| Compute bomb | `compute_units: 1500000`, ComputeBudget auto-injected | `max_compute_units` |

**Important:** unwhitelisted program prompt must use `slippage_bps ≤ 200`, otherwise `max_slippage` fires first (rules run in order).

### Rate limit / backoff

- Normal interval: `setInterval(agentLoop, 8000)` — 8 seconds
- On RATE_LIMITED / 502 / 503: clear interval → sleep 20 seconds → restart interval
- On API_KEY_EXPIRED: clear interval, show error, stop

### Design tokens (CSS vars)

```
--sak-bg: #0a0a0f      (near-black, blue-shifted)
--sak-green: #00ff88   (ALLOWED, brand)
--sak-red: #ff3366     (BLOCKED, critical)
--sak-orange: #ff9900  (HIGH severity / JS fallback badge)
--sak-purple: #7c3aed  (AI agent node / ⚙ Rust badge)
```

Fonts: Surgena (local OTF) + JetBrains Mono (Google Fonts CDN).

---

## rules.yaml

```yaml
rules:
  - name: max_slippage       type: slippage_check      max_bps: 200
  - name: allowed_programs   type: program_whitelist    programs: [Jupiter v6, Orca, System, SPL Token, ATA, ComputeBudget]
  - name: max_account_drain  type: drain_check          max_lamports: 1000000000  (1 SOL)
  - name: max_accounts       type: account_count_check  max_count: 20
  - name: max_compute_units  type: compute_units_check  max_units: 1400000
  - name: max_priority_fee   type: priority_fee_check   max_microlamports: 1000000
  - name: min_transfer_value type: min_transfer_lamports min_lamports: 1
```

---

## Environment Variables

```bash
# .env (gitignored — actual secrets)
YELLOWSTONE_TOKEN=<real token>

# .env.example (committed — placeholder)
YELLOWSTONE_ENDPOINT=https://sol-devnet-yellowstone-grpc.rpcfast.com:443
YELLOWSTONE_TOKEN=your_token_here
```

Never hardcode the actual token. The `.env` file is gitignored — do not commit it.

---

## Build & Test

```bash
# Build everything
cargo build --workspace

# Run Guardian tests (20/20 must pass)
cargo test -p sak-guardian

# Run the demo
cargo run -p race-server          # Terminal 1 — starts on :3001
cd demo/race-ui && python3 -m http.server 4000  # Terminal 2 — serves HTML on :4000
# Open http://localhost:4000 in browser
```

---

## Key Technical Decisions & Gotchas

1. **ChainEvent is an enum, not a struct.** Originally a struct `{ slot: u64, kind: EventKind }`. Converted to a flat enum with slot embedded in each variant to satisfy `ChainEvent::SlotUpdate` variant syntax required by Pillar 1.

2. **`yellowstone-grpc-proto` must use `default-features = false`.** The default features include `convert` which pulls `solana-sdk ~2.1.1`, conflicting with `light-compressed-token`'s `solana-sdk ^2.2`.

3. **`SubscribeRequestFilterSlots` has an undocumented `interslot_updates` field.** Always use `..Default::default()` when constructing it.

4. **Only `VersionedMessage::Legacy` is supported in the simulator.** V0 messages panic or return an error. The evil corpus tests only use legacy messages.

5. **Guardian tests import from `sak-core`** (`Decision`, `TxMeta`) and `sak-guardian` (`Guardian`, `Rule`). They do not use `ChainEvent` or `EventKind` at all.

6. **DrainCheck uses LiteSVM pre/post balance diff on the simulated path.** On the raw path it parses bincode system program instructions (4-byte discriminant + 8-byte u64 LE).

7. **ComputeBudget uses 1-byte discriminants** (not bincode): `0x02` = SetComputeUnitLimit, `0x03` = SetComputeUnitPrice.

8. **CoinGecko cannot be called from the browser on localhost.** Proxied through race-server `/sol-price` with 60s cache. CORS is added via `tower-http::CorsLayer`.

9. **The race-server uses `tower-http` v0.5** (not 0.6) to match axum 0.7 compatibility. Both exist in the lockfile due to transitive deps but the race-server explicitly uses 0.5.

10. **`sak-state` is a stub.** `ZkState` uses `HashMap` in memory. `flush_to_zk()` logs but does nothing. Light Protocol SDK is pinned at `"*"` to avoid conflicts.

11. **`evaluate_raw` receives `&[(u8, &[u8])]` — lifetime trick.** Build `Vec<Vec<u8>>` (owned data) first, then build `Vec<(u8, &[u8])>` borrowing from it, then pass `&raw_ixs`. Both vecs must remain in scope for the duration of the call.

12. **DrainCheck on the Raw path requires the System Program in account_keys at the instruction's program_id_index.** It parses instruction data, not account balances. If intent has `program_ids: ["11111..."]` and `amount_lamports > 0`, the handler encodes a real System Transfer discriminant so the rule can parse it.

13. **ComputeUnitsCheck requires ComputeBudget in account_keys.** If the intent declares `compute_units > 0` but ComputeBudget is not in `program_ids`, race-server auto-injects it into account_keys with the encoded SetComputeUnitLimit instruction.

14. **`tracing_subscriber::fmt::init()` defaults to WARN level** if `RUST_LOG` is not set. All SAK servers now use `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))` so logs are visible by default without setting env vars.

15. **tx-generator broken pipe.** Using `println!` in the subprocess panics when race-server closes its stdout pipe. Fixed: use `writeln!(io::stdout(), …)`, check `Result`, `break` on error.

16. **`via_backend` flag on UI entries.** When `evaluateWithBackend()` gets a successful response from `/evaluate`, it sets `entry.via_backend = true`. The log card renders a purple **⚙ Rust** badge. If the server is unreachable, `evaluateIntent()` (JS fallback) is called instead and an orange **JS fallback** badge is shown.

17. **Yellowstone TLS — `tls-native-roots` feature is required.** Without it, `ClientTlsConfig::with_native_roots()` is a no-op (guarded by `#[cfg(feature = "tls-native-roots")]` inside tonic). The result is a silent TLS failure: `"gRPC transport error: transport error"`. Always list both `"tls"` and `"tls-native-roots"` in tonic's Cargo features and call `.tls_config(ClientTlsConfig::new().with_native_roots())?` explicitly.

18. **WebSocket carries two message types; route by `type` field.** tx-generator events have no `type` field. Yellowstone slot events have `"type":"slot_update"`. The `connectSlotWS()` handler checks `parsed.type === 'slot_update'` and ignores everything else. The main NVIDIA agent loop does NOT read from the WebSocket — it generates intents independently via the NIM API.

19. **`YELLOWSTONE_TOKEN` missing = graceful degradation, not panic.** race-server checks the env var at startup. If empty: logs a warning, skips spawning the Reflex Engine tasks. All REST endpoints (`/evaluate`, `/sol-price`, `/feedback`) still work. The slot counter in the UI stays `—`. This is intentional — the demo should not crash when run without a Yellowstone subscription.

20. **Never use `fetch('/api/...')` (leading slash) on static hosts.** The path resolves from `window.location.origin` only. On `https://user.github.io/repo/`, that becomes `https://user.github.io/api/...` (404), not under `/repo/`. Always use `API_BASE + '/api/...'` with `API_BASE` set to the race-server origin on `*.github.io`, `*.pages.dev`, or via `<meta name="sak-api-base" content="https://…">`.

21. **`sak-api-base` must not point at the static site itself.** If the meta URL’s origin equals the page origin on a static host, the demo JS treats it as a mistake and forces the Railway `race-server` base URL instead.

22. **Cloudflare Pages hostnames** are per project. Production is typically `https://<project-name>.pages.dev/` after the first deployment. Preview builds use `https://<deployment-id>.<project-name>.pages.dev/`. Underscores are **not** allowed in Pages project names (use hyphens, e.g. `sak-devnet-test`).

23. **Tx generator on Railway stays off in production** when `RAILWAY_ENVIRONMENT` is set, unless `ENABLE_TX_GENERATOR=true`. The subprocess spawns `cargo run` for `demo/tx-generator` — not appropriate for slim deploy images. This is expected in logs, not a failure.

---

## Crate Dependency Graph

```
sak-bin ──► sak-sdk ──► sak-guardian ──► sak-core
         └──► sak-reflex ──► sak-core     │
              sak-state ──────────────────┘

race-server ──► sak-guardian ──► sak-core     (evaluate_raw → POST /evaluate)
             ├──► sak-reflex  ──► sak-core     (Yellowstone slot stream → /ws broadcast)
             └──► sak-core

tx-generator ──► sak-guardian ──► sak-core    (evaluate full tx via LiteSVM)
              └──► sak-core
```

---

## What Is Complete vs Stub

| Component | Status | Notes |
|---|---|---|
| `sak-guardian` | Full | 20 tests passing, LiteSVM simulation working |
| `sak-reflex` | Full | Real Yellowstone gRPC connection + reconnect |
| `sak-state` | Stub | In-memory only, Light Protocol not wired |
| `sak-sdk` | Full | Kernel API wraps all pillars |
| `sak-bin` | Full | Spawns Reflex Engine, logs slots |
| `race-server` | Full | WebSocket + feedback + SOL price proxy + `/evaluate` + Yellowstone slot broadcast |
| `race-ui` | Full | Standalone HTML demo, NVIDIA NIM → real `/evaluate` backend, JS fallback, live slot counter |
| Demo recording | Pending | |
| Deployment | Partial | See **Hosted demo, Cloudflare & Railway** below — waitlist (`sak_ui-1`) and static Guardian demo are split across two Pages projects; `race-server` on Railway. |

---

## Hosted demo, Cloudflare & Railway

This section documents how the **live NVIDIA / Guardian demo**, the **waitlist / marketing site**, and **`race-server`** are wired. Read this before changing URLs, env vars, or deploy scripts.

### Repositories (two siblings)

| Repo | Path (typical) | Role |
|------|----------------|------|
| **sak** (this workspace) | `…/solana/sak` | Rust workspace + root **`index.html`** static Guardian demo (NVIDIA key gate + dashboard). |
| **sak_ui-1** | `…/solana/sak_ui-1` | React **waitlist / landing** (Bun build → `dist/`). Not part of this git repo — clone separately. |

### Cloudflare Pages (two projects)

| Pages project | Public URL (production) | Contents |
|---------------|-------------------------|----------|
| **`sak`** | [https://sak-d89.pages.dev](https://sak-d89.pages.dev) | **`sak_ui-1`** only: `bun run build` → `npx wrangler pages deploy dist --project-name sak`. SPA fallback `_redirects`: `/* /index.html 200`. |
| **`sak-devnet-test`** | [https://sak-devnet-test.pages.dev](https://sak-devnet-test.pages.dev) | Static bundle from **`sak`**: `index.html` + `fonts/` (see scripts below). NVIDIA + `/evaluate` call **Railway** `race-server`. |

**Naming:** Cloudflare does not allow underscores in Pages project names; the devnet demo project is **`sak-devnet-test`**, not `sak_devnet_test`.

**Waitlist → demo:** In `sak_ui-1`, `src/siteUrls.ts` exports `SAK_DEVNET_TEST_DEMO_URL` (default `https://sak-devnet-test.pages.dev/`). **Devnet Test** in `src/components/landing/Navbar.tsx` opens that URL in a new tab. Update `siteUrls.ts` if Cloudflare shows a different production hostname.

### Railway — `race-server`

- **Example URL:** `https://race-server-production-c5c9.up.railway.app` (replace if your service is renamed).
- **Purpose:** `/health`, `POST /evaluate` (Rust Guardian), `GET /sol-price`, `GET|POST /api/nvidia/*` (proxy to NVIDIA), `GET /ws` (slot broadcast when Reflex runs), `POST /feedback`, CORS.
- **Useful env vars:** `CORS_ALLOWED_ORIGINS` (comma list or `*` for demos), `NVIDIA_API_KEY` if proxying without browser key, `HELIUS_API_KEY` or `YELLOWSTONE_TOKEN` (+ `YELLOWSTONE_ENDPOINT` / `GEYSER_ENDPOINT`) to enable **Reflex** / slot stream, `ENABLE_TX_GENERATOR` / `RAILWAY_ENVIRONMENT` (tx generator off on Railway by default).
- **Typical log lines (normal):** “Tx generator disabled…”, “HELIUS_API_KEY / YELLOWSTONE_TOKEN not set — Reflex Engine disabled”, “listening on `0.0.0.0:8080`” — not fatal; REST + NVIDIA proxy still work without Reflex.

### Root `index.html` & `demo/race-ui/index.html` (static demo)

Keep these in sync when changing demo UX or API wiring.

| Mechanism | Behavior |
|-----------|----------|
| **`API_BASE`** | Resolves backend for `fetch`. Reads `<meta name=”sak-api-base”>`. On `github.io`, `*.github.io`, `*.pages.dev`, uses Railway default if meta empty; if meta’s **origin equals the page origin** on those hosts, ignores meta and uses Railway (avoids “API base = Pages URL” 404s). Otherwise `location.origin` when served behind local `race-server`. |
| **`sak-landing-url` meta** | Default in root `index.html`: `https://sak-d89.pages.dev` — **Landing page** / gate **Marketing site →** link back to waitlist. `wireMarketingLandingLinks()` also sets landing to `origin + ‘/’` when the path starts with `/guardian` and meta is empty (same-site subpath hosting — optional future use). |
| **Demo Mode** | “Try Demo — no API key needed” button on the gate screen sets `isDemoMode = true`. Skips NVIDIA API; uses scripted attack/valid cycle (same FALLBACK_ATTACKS / FALLBACK_VALID). Guardian `/evaluate` still calls real Rust. Orange “Demo Mode” badge shown in header. `initDashboard()` resets all counters and reads `isDemoMode` to show/hide the badge. Stop demo resets `isDemoMode = false`. |
| **Stop demo** | Header button: clears agent interval, sets `isDemoMode = false`, clears `storedApiKey` + gate input, **Phantom `disconnect()`**, resets wallet UI, runs `__sakSlotCleanup()`, fades back to **gate** screen. |
| **Slot WebSocket** | `window.__sakStartSlotWs` / `window.__sakSlotCleanup` — started on load and again from `initDashboard()`; cleaned on Stop demo so the socket does not reconnect forever in the background. |
| **Squads Layer 2 panel** | `initSquadsPolicy()` called from `initDashboard()`. POSTs to `/squads/create-agent-wallet`. `renderSquadsPolicy(data)` populates `#squadsContent` with address, $10 USDC/tx limit, Solscan ↗ and Squads ↗ links, and the `api_note`. Falls back to “Offline” state if race-server unreachable. |

### Deploy scripts (`sak/`)

| Script | What it does |
|--------|----------------|
| **`scripts/bundle-static-demo.sh`** | Copies `index.html` and `fonts/` → **`.pages-out/`** (gitignored). |
| **`scripts/deploy-devnet-demo.sh`** | Runs the bundle script, then `npx wrangler pages deploy .pages-out --project-name "${CF_PAGES_DEMO_PROJECT:-sak-devnet-test}"`. Requires `wrangler login` (or API token) on the machine running deploy. |

**First-time Cloudflare project:** `npx wrangler pages project create sak-devnet-test --production-branch=main` then deploy.

### GitHub Pages (optional)

- Workflow: **`.github/workflows/deploy-github-pages.yml`** — builds `_site` from `index.html` + `fonts/`, deploys via GitHub Actions **Pages** (repo Settings → Pages → source **GitHub Actions**).
- If Pages source stays **“Deploy from branch”**, push updated `index.html` to that branch instead.

### Local dev (`demo/race-ui`)

- **`vite.config.ts`** proxies `/evaluate`, `/squads`, `/feedback`, `/sol-price`, `/health`, `/ws` to `127.0.0.1:3001` (local `race-server`).
- **`demo/race-ui/index.html`** mirrors root demo; `sak-api-base` meta is often empty so `API_BASE` follows `localhost` during Vite.

### Future checklist (for maintainers)

1. **After editing root `index.html`:** redeploy **`sak-devnet-test`** (and GitHub Pages / other mirrors if used). Re-run **`sak_ui-1` build + deploy `sak`** only if you still copy demo into waitlist (currently **not** bundled — demo is separate project).
2. **CORS:** If Railway `CORS_ALLOWED_ORIGINS` is a strict list, add `https://sak-devnet-test.pages.dev` and `https://sak-d89.pages.dev` as needed.
3. **Secrets:** Never commit real `NVIDIA` / `HELIUS` keys; keep **`.env.example`** as placeholders only; rotate any key that was ever committed.
4. **CI for `sak_ui-1`:** If a workflow ever needs the Guardian HTML from this repo, check out **both** repos as siblings (`sak` next to `sak_ui-1`) or use a submodule / artifact.
5. **Custom domains:** Point DNS at Cloudflare Pages for either project; update `siteUrls.ts`, `sak-landing-url`, and CORS allowlists accordingly.
6. **Squads endpoint:** `/squads/create-agent-wallet` is a mock returning a pre-configured devnet address. To make it real, run `@squads-protocol/multisig` once on devnet, fund the keypair, and hardcode the resulting PDA. Update the Solscan link accordingly.

---

## POST /squads/create-agent-wallet

**Layer 2 spending-limit policy** via Squads v4 smart accounts. The multisig was created on devnet via `scripts/create-squads-account.ts`.

**Request body:**
```json
{
  "agent_name": "SAK Demo Agent",
  "spending_limit_usdc": 10
}
```

**Response:**
```json
{
  "status": "created",
  "smart_account": "HzaSqyyW5kuGyGFndRhZjx5h24TB79ZUsxEMPUsKSfoX",
  "config_authority": "2bzdLiLZdKRgb1zMdndTbDEgtbPwLepfjNPPCQrawaoZ",
  "spending_limit_usdc": 10.0,
  "spending_limit_atoms": 10000000,
  "program_id": "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf",
  "explorer_url": "https://solscan.io/account/7YzHDnz...?cluster=devnet",
  "squads_app_url": "https://v4.squads.so/multisigs/7YzHDnz...",
  "api_note": "Squads API integration — on-chain creation requires a funded keypair...",
  "sdk_snippet": "// @squads-protocol/multisig ... multisigCreateV2 + spendingLimitCreate"
}
```

**Squads v4 program ID:** `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`

**To make it real:** install `@squads-protocol/multisig`, fund a devnet keypair (~0.05 SOL), call `multisigCreateV2` + `spendingLimitCreate`, hardcode the resulting PDA. The response structure doesn't change — only the addresses become real.

---

## Demo Mode (no API key)

Added to both `index.html` and `demo/race-ui/index.html`.

**Gate screen:** "Try Demo — no API key needed" button below the "Spawn AI Agent" button.

**Behaviour:**
- Sets `isDemoMode = true`, skips NVIDIA `callNVIDIA()` entirely
- Uses scripted `FALLBACK_ATTACKS` / `FALLBACK_VALID` intents with a 300–700ms simulated delay
- `evaluateWithBackend()` still POSTs to `/evaluate` — real Rust Guardian runs on every intent
- Orange `Demo Mode` badge shown in dashboard header
- `initDashboard()` resets all counters and reads `isDemoMode` to show/hide the badge
- `stopDemoAndReturnToGate()` resets `isDemoMode = false`

**Key JS variable:** `let isDemoMode = false;` (declared before `validateKey`).

**Gotcha 24:** Demo Mode bypasses the NVIDIA key validation but still calls `/evaluate` on the backend. If race-server is offline, the JS `evaluateIntent()` fallback runs instead and shows an orange "JS fallback" badge — this is correct and intentional.

---

## Flow Diagram — Updated Architecture

Both `index.html` and `demo/race-ui/index.html` now show a 4-node flow:

```
AI Agent → Guardian → Squads → Solana
```

Previously: `AI Agent → Reflex → Guardian → Solana`

**Reason for change:** The flow diagram represents the *transaction flow* (intent → evaluation → policy → chain), not the *subscription flow* (Yellowstone events). Reflex Engine is a background subscriber — it doesn't sit in the transaction path.

**Node mapping in `animateFlow()`:**

| Node | Icon | Color (active) | Stage |
|------|------|---------------|-------|
| AI Agent | `bot` | purple | 0 |
| Guardian | `shield` | orange → red/green | 1 |
| Squads | `lock` | blue → green | 2 (blocked: `x` / red) |
| Solana | `circle` | #9945ff | 3 (blocked: dimmed) |

**Blocked path:** Guardian rejects → shows `Blocked` node at position 3 with red styling, Squads and Solana nodes dim out (never reached).

**Trace card children index map** (both files):

| Index | Element |
|-------|---------|
| 0 | Agent node |
| 1 | Seg: Agent→Guardian |
| 2 | Guardian node |
| 3 | Seg: Guardian→Squads/Blocked |
| 4 | Squads or Blocked node |
| 5 | Seg: Squads→Solana |
| 6 | Solana node |

Animation sequence:
- 350ms: seg1 orange, node2 (Guardian) becomes `sim...`
- 750ms: node2 resolves (red/green), seg3 + node4 (Squads/Blocked) activate
- 1150ms: node4 (Squads) resolves `$10 ✓`, seg5 green, node6 (Solana) activates *(allowed only)*
- 1100/1600ms: `.trace-outcome` fades in with rule/sig info

---

## What Is Complete vs Stub (Updated)

| Component | Status | Notes |
|---|---|---|
| `sak-guardian` | Full | 20 tests passing, LiteSVM simulation working |
| `sak-reflex` | Full | Real Yellowstone gRPC connection + reconnect |
| `sak-state` | **Stub** | In-memory only, Light Protocol not wired. README updated to reflect this. |
| `sak-sdk` | Full | Kernel API wraps all pillars |
| `sak-bin` | Full | Spawns Reflex Engine, logs slots |
| `race-server` | Full | WebSocket + feedback + SOL price proxy + `/evaluate` + `/squads/create-agent-wallet` + Yellowstone slot broadcast |
| `race-ui` | Full | Demo Mode, NVIDIA NIM → real `/evaluate`, Squads panel, SVG architecture diagram, live slot counter |
| Demo recording | Pending | |
| Deployment | Partial | See **Hosted demo, Cloudflare & Railway** |

**README accuracy:** Pillar 3 now marked `🔧 Stub` in both the features table and build phases table.

---

## Gotcha 24 — Demo Mode + JS fallback coexist

When `isDemoMode === true` AND `/evaluate` is unreachable, the UI calls `evaluateIntent()` (JS fallback) and shows an orange "JS fallback" badge. The demo still runs — it just evaluates in the browser instead of calling Rust. This is intentional.

## Gotcha 25 — `initDashboard()` now resets counters

`initDashboard()` now explicitly resets all dashboard state:
```javascript
txLog = []; txCount = 0; allowedCount = 0; blockedCount = 0;
totalPrevented = 0; feedbackCorrect = 0; feedbackWrong = 0; falsePositives = 0;
```
This prevents stale counts when the user stops the demo and restarts it (previously counters persisted across demo sessions).

---
