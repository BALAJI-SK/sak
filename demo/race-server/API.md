# SAK Race Server — API Reference

> Real-time transaction simulation dashboard API.
> Listens on `http://localhost:3001`.

---

## Table of Contents

1. [WebSocket — Transaction Stream](#1-websocket--transaction-stream)
2. [POST /feedback — Submit Feedback](#2-post-feedback--submit-feedback)
3. [GET /feedback/summary — Feedback Statistics](#3-get-feedbacksummary--feedback-statistics)
4. [Data Types](#4-data-types)
5. [Error Handling](#5-error-handling)
6. [Quick Start](#6-quick-start)

---

## 1. WebSocket — Transaction Stream

```
GET /ws
```

Upgrades to WebSocket and streams live transaction log entries as JSON.
Each entry represents a transaction evaluated by the Guardian.

### Connection

```javascript
const ws = new WebSocket("ws://localhost:3001/ws");

ws.onmessage = (event) => {
  const tx = JSON.parse(event.data);
  console.log(tx.decision, tx.attack_type);
};
```

### Message: Rejected Transaction

```json
{
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "timestamp": "2026-05-07T16:21:55.123456Z",
  "decision": "rejected",
  "rule": "max_slippage",
  "reason": "9900bps exceeds maximum 200bps",
  "attack_type": "99% Slippage Swap",
  "description": "Agent tried to swap 0.01 SOL with 99% slippage tolerance",
  "severity": "critical",
  "simulated_loss_usd": 49.86,
  "simulation_time_ms": 43
}
```

### Message: Allowed Transaction

```json
{
  "id": "b2c3d4e5-f6a7-8901-bcde-f12345678901",
  "timestamp": "2026-05-07T16:22:01.654321Z",
  "decision": "allowed",
  "attack_type": "Valid Swap",
  "description": "Agent executed valid swap with 1% slippage tolerance",
  "severity": "none",
  "simulation_time_ms": 32
}
```

### Fields

| Field | Type | Always | Description |
|-------|------|:------:|-------------|
| `id` | `string` (UUIDv4) | yes | Unique transaction identifier |
| `timestamp` | `string` (RFC 3339) | yes | Generation timestamp |
| `decision` | `"allowed"` \| `"rejected"` | yes | Guardian decision |
| `rule` | `string` \| `null` | rejected only | Triggered rule name |
| `reason` | `string` \| `null` | rejected only | Human-readable reason |
| `attack_type` | `string` | yes | Label (e.g. `"Drain Balance"`) |
| `description` | `string` | yes | What the agent attempted |
| `severity` | `"critical"` \| `"high"` \| `"medium"` \| `"low"` \| `"none"` | yes | Threat severity |
| `simulated_loss_usd` | `number` \| `null` | drain/slippage only | Estimated prevented loss |
| `simulation_time_ms` | `integer` | yes | Simulation duration |

---

## 2. POST /feedback — Submit Feedback

```
POST /feedback
Content-Type: application/json
```

Records user feedback on a Guardian decision for accuracy tracking.

### Request Body

```json
{
  "timestamp": "2026-05-07T16:21:55Z",
  "decision": "rejected",
  "rule": "max_slippage",
  "description": "99% Slippage Swap",
  "stars": 1,
  "verdict": "Wrong"
}
```

### Fields

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `timestamp` | `string` (RFC 3339) | yes | Timestamp of original transaction |
| `decision` | `"allowed"` \| `"rejected"` | yes | Original Guardian decision |
| `rule` | `string` \| `null` | no | Rule that triggered |
| `description` | `string` \| `null` | no | Transaction description |
| `stars` | `integer` (1–5) | yes | Star rating |
| `verdict` | `"Correct"` \| `"Wrong"` \| `"Neutral"` | yes | Derived verdict |

**Star → Verdict mapping:**

| Stars | Verdict |
|:-----:|---------|
| 1–2 | `Wrong` |
| 3 | `Neutral` |
| 4–5 | `Correct` |

### Response

```
200 OK
Content-Type: text/plain

recorded
```

### Example (curl)

```bash
curl -X POST http://localhost:3001/feedback \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "2026-05-07T16:21:55Z",
    "decision": "rejected",
    "rule": "max_slippage",
    "description": "99% Slippage Swap",
    "stars": 1,
    "verdict": "Wrong"
  }'
```

---

## 3. GET /feedback/summary — Feedback Statistics

```
GET /feedback/summary
```

Returns aggregate feedback statistics.

### Response

```json
{
  "total": 18,
  "correct": 14,
  "wrong": 4,
  "accuracy": 77.8
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `total` | `integer` | Total feedback entries submitted |
| `correct` | `integer` | Count of `Correct` verdicts |
| `wrong` | `integer` | Count of `Wrong` verdicts |
| `accuracy` | `number` | Percentage = `(correct / total) × 100` (0 if empty) |

Note: `Neutral` verdicts contribute to `total` but not to `correct` or `wrong`, so `correct + wrong` may be less than `total`.

### Example (curl)

```bash
curl http://localhost:3001/feedback/summary
```

---

## 4. Data Types

### Severity Levels

| Level | Meaning | Color |
|-------|---------|-------|
| `critical` | Drain attacks, 99% slippage | `#ff3366` |
| `high` | Unknown programs, below rent | `#ff9900` |
| `medium` | Excessive compute/priority fees | `#ffd700` |
| `low` | Zero amount transfers | `#8888aa` |
| `none` | Valid transactions | `#00ff88` |

### Guardian Rules

| Rule | Trigger |
|------|---------|
| `max_slippage` | Slippage tolerance exceeds 200 bps |
| `max_account_drain` | Single transfer > 1 SOL |
| `allowed_programs` | Unknown/unregistered program ID |
| `max_compute_units` | Compute budget exceeds limit |
| `min_transfer_value` | Transfer amount is 0 |
| `max_priority_fee` | Priority fee > 1,000,000 µlamports |
| `pre_sign_simulation` | Simulation failed pre-flight |

### Transaction Patterns (tx-generator)

| Pattern | Attack Type | Severity | Loss |
|---------|-------------|:--------:|:----:|
| Slippage99 | 99% Slippage Swap | critical | ~$49.86 |
| DrainBalance | Drain Balance | critical | ~$498.50 |
| UnknownProgram | Unknown Program | high | — |
| ExcessiveCompute | Excessive Compute | medium | — |
| ZeroAmount | Zero Amount | low | — |
| ExcessivePriorityFee | Excessive Priority Fee | medium | — |
| ValidSwap | Valid Swap | none | — |
| ValidTransfer | Valid Transfer | none | — |

---

## 5. Error Handling

The race-server has minimal error handling (demo service):

- **Malformed JSON** on `POST /feedback` → Axum returns 422 Unprocessable Entity
- **WebSocket disconnect** → silently drops connection
- **No routes matched** → Axum returns 404

There are no authentication or rate limits — this is a local demo server.

---

## 6. Quick Start

### Prerequisites

- Rust toolchain (1.80+)
- Node.js (20+) for the UI

### Start the server

```bash
# From project root
cargo run -p race-server
```

The server starts on `ws://localhost:3001` and spawns the tx-generator automatically.
Transactions are generated every ~2 seconds.

### Start the UI

```bash
cd demo/race-ui
npm run dev
```

Open http://localhost:3000 in a browser.

### Verify the API

```bash
# Feedback summary (will show 0 until you submit feedback)
curl http://localhost:3001/feedback/summary

# Submit feedback
curl -X POST http://localhost:3001/feedback \
  -H "Content-Type: application/json" \
  -d '{"timestamp":"2026-05-07T16:21:55Z","decision":"rejected","rule":"max_slippage","description":"Test","stars":1,"verdict":"Wrong"}'

# Check summary again
curl http://localhost:3001/feedback/summary
```

### OpenAPI Spec

An OpenAPI 3.0 specification is available at [`openapi.yaml`](openapi.yaml).
