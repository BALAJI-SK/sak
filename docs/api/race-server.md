# race-server — Demo HTTP + WebSocket API

Axum server serving the SAK Guardian demo UI. Proxies NVIDIA NIM, evaluates transaction intents against real Rust Guardian rules, streams Solana slot updates via WebSocket, and manages user feedback.

**Port:** 3001 (local) / 8080 (Railway container)

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` | Liveness check |
| POST | `/evaluate` | Evaluate a transaction intent against Guardian rules |
| GET | `/sol-price` | SOL/USD price (CoinGecko proxy, 60s cache) |
| POST | `/feedback` | Submit user feedback on a Guardian decision |
| GET | `/feedback/summary` | Aggregate feedback stats |
| POST | `/squads/create-agent-wallet` | Demo Squads multisig info |
| GET | `/ws` | WebSocket — slot updates + tx-generator stream |
| GET | `/api/nvidia/models` | Proxy to NVIDIA NIM models list |
| POST | `/api/nvidia/chat/completions` | Proxy to NVIDIA NIM chat completions |

---

### POST /evaluate

Core evaluation endpoint. Maps a high-level intent to instructions and runs `sak-guardian::evaluate_raw`.

**Request:**

```json
{
  "slippage_bps": 9900,
  "amount_lamports": 100000000,
  "program_ids": ["JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"],
  "compute_units": 0,
  "description": "maximize USDC to SOL swap output"
}
```

| Field | Type | Description |
|---|---|---|
| `slippage_bps` | u64 | Agent-declared slippage in basis points |
| `amount_lamports` | u64 | Transfer amount in lamports |
| `program_ids` | [String] | Program IDs the transaction will invoke |
| `compute_units` | u64 | Compute unit budget requested |
| `description` | String | Human-readable intent (for audit trail) |

**Response:**

```json
{
  "decision": "rejected",
  "rule": "max_slippage",
  "reason": "slippage 9900bps exceeds max 200bps",
  "attack_type": "99% Slippage Swap",
  "severity": "critical",
  "simulation_time_ms": 19
}
```

| Field | Type | Description |
|---|---|---|
| `decision` | `"allowed"` \| `"rejected"` | Guardian verdict |
| `rule` | String | Rule name that triggered rejection (null if allowed) |
| `reason` | String | Human-readable rejection reason |
| `attack_type` | String | Classification label |
| `severity` | `"none"` \| `"low"` \| `"medium"` \| `"high"` \| `"critical"` | Severity rating |
| `simulation_time_ms` | u64 | Wall-clock time for evaluation |

**Guardian Rules Applied** (in order):

1. `SlippageCheck` — max 200 bps
2. `ProgramWhitelist` — Jupiter v6/v4, Orca, Raydium, SPL Token, ATA, System, ComputeBudget, SysvarRent
3. `DrainCheck` — max 1 SOL
4. `ComputeUnitsCheck` — max 1,400,000 CU
5. `PriorityFeeCheck` — max 1,000,000 microlamports
6. `MinTransferLamports` — min 1 lamport

**Layer 2 Squads override:** If Guardian allows but `amount_lamports > 100_000_000` (0.1 SOL notional in this demo), the response flips to `"rejected"` with rule `"squads_spending_limit"`, so conservative 0.1 SOL swaps still pass while larger allowed transfers illustrate on-chain policy.

**Attack Classification:**

| Rule | Trigger | Attack Type | Severity |
|---|---|---|---|
| `max_slippage` | ≥ 9000 bps | 99% Slippage Swap | critical |
| `max_slippage` | ≥ 5000 bps | High-Slippage Swap | critical |
| `max_slippage` | < 5000 bps | High-Slippage Swap | high |
| `max_account_drain` | > 5 SOL | Drain Balance | critical |
| `max_account_drain` | ≤ 5 SOL | Drain Balance | high |
| `allowed_programs` | any | Unwhitelisted Program | medium |
| `max_compute_units` | any | Compute Bomb | medium |
| `max_priority_fee` | any | Priority Fee Bomb | medium |

---

### GET /sol-price

**Response:**

```json
{ "usd": 162.40 }
```

Cached in-memory for 60 seconds. Falls back to last known price (default 150.0) on upstream fetch error. Upstream: CoinGecko `simple/price?ids=solana`.

---

### POST /feedback

**Request:**

```json
{
  "timestamp": "2026-05-11T12:00:00Z",
  "decision": "rejected",
  "rule": "max_slippage",
  "description": "maximize swap output",
  "stars": 5,
  "verdict": "correct"
}
```

`verdict` accepts `"correct"`, `"wrong"`, `"neutral"` (case-insensitive), or serde enum format `{"Correct": null}`.

**Response:** plain text `"recorded"`

Stored in-memory (`Arc<Mutex<Vec<GuardianFeedback>>>`). Not persisted across restarts.

---

### GET /feedback/summary

**Response:**

```json
{
  "total": 42,
  "correct": 38,
  "wrong": 4,
  "accuracy": 90.5
}
```

---

### POST /squads/create-agent-wallet

**Request:**

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
  "explorer_url": "https://solscan.io/account/...?cluster=devnet",
  "squads_app_url": "https://v4.squads.so/multisigs/...",
  "api_note": "Squads API integration — on-chain creation requires a funded keypair...",
  "sdk_snippet": "// @squads-protocol/multisig ..."
}
```

Does NOT create an on-chain account. Returns hardcoded devnet addresses configured via env vars:

| Env Var | Purpose |
|---|---|
| `SQUADS_SMART_ACCOUNT` | Squads multisig address (fallback: hardcoded devnet address) |
| `SQUADS_CONFIG_AUTHORITY` | Multisig config authority (fallback: devnet creator keypair) |

---

### GET /ws

WebSocket upgrade. Each connected client receives JSON messages broadcast from two sources:

**Slot updates** (from Reflex Engine / Yellowstone Geyser):
```json
{"type": "slot_update", "slot": 312345678}
```

**Tx-generator events** (from `demo/tx-generator` subprocess):
```
{"id":"tx-...","timestamp":"...","decision":"rejected","rule":"max_slippage",...}
```

No authentication. Broadcast channel capacity: 1024 messages.

---

### GET /health

**Response:** plain text `"ok"`

---

## Environment Variables

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `YELLOWSTONE_TOKEN` | No | — | Enables Reflex Engine (slot stream via WS). Missing = graceful degradation |
| `SQUADS_SMART_ACCOUNT` | No | Hardcoded devnet | Squads multisig address |
| `SQUADS_CONFIG_AUTHORITY` | No | Hardcoded devnet | Squads config authority keypair |
| `RAILWAY_ENVIRONMENT` | N/A | — | Set by Railway — disables tx-generator subprocess |
| `ENABLE_TX_GENERATOR` | No | `false` | Force-enable tx-generator on Railway |
| `RUST_LOG` | No | `info` | Log level filter |

## Notes

- CORS allows all origins (`CorsLayer::permissive()`)
- All state is in-memory — restarting the server clears feedback and price cache
- NVIDIA proxy forwards `Authorization`, `Accept`, and `Content-Type` headers upstream
- Tx-generator subprocess runs only when not on Railway (or `ENABLE_TX_GENERATOR=true`)
