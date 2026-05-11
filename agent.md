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
├── rules.yaml                  # Guardian rule definitions (read at runtime)
├── .env                        # gitignored — real secrets go here
├── .env.example                # committed placeholder values
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
**Framework:** Axum 0.7 + tokio-tungstenite

### Endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/ws` | WebSocket — streams JSON transaction events |
| GET | `/sol-price` | Returns `{"usd": <f64>}`. Proxies CoinGecko with 60s server-side cache. |
| POST | `/feedback` | Accepts `GuardianFeedback` JSON, stores in memory |
| GET | `/feedback/summary` | Returns `{total, correct, wrong, accuracy}` |
| POST | `/evaluate` | **Real Rust Guardian evaluation** — takes intent JSON, runs `sak-guardian::evaluate_raw`, returns decision |

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

**File:** `demo/race-ui/index.html` — standalone HTML file with inline CSS + JS. NOT a Vite/React app.

**Served on:** port 4000 (static file server, e.g. `python3 -m http.server 4000`)  
**Connects to:** `http://localhost:3001` for price, evaluation, and feedback

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

### Key JS functions

- `fetchSolPrice()` — calls `http://localhost:3001/sol-price` (proxied, not CoinGecko directly)
- `agentLoop()` / `_agentLoopBody()` — every 8 seconds: fetches SOL price → calls NVIDIA NIM → `evaluateWithBackend()` → updates UI
- `callNVIDIA(prompt)` — calls NVIDIA NIM API (`minimaxai/minimax-m2.7` model) via Vite proxy at `/api/nvidia`
- `agentRunning` guard — prevents concurrent `agentLoop` invocations; interval cleared during rate-limit backoff

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

---

## Crate Dependency Graph

```
sak-bin ──► sak-sdk ──► sak-guardian ──► sak-core
         └──► sak-reflex ──► sak-core     │
              sak-state ──────────────────┘

race-server ──► sak-guardian ──► sak-core     (evaluate_raw → POST /evaluate)
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
| `race-server` | Full | WebSocket + feedback + SOL price proxy + `/evaluate` (real Rust Guardian) |
| `race-ui` | Full | Standalone HTML demo, NVIDIA NIM → real `/evaluate` backend, JS fallback |
| Demo recording | Pending | |
| Deployment | Pending | |
