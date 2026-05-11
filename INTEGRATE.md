# SAK HTTP API — No Rust Required

SAK's Guardian evaluates transactions over HTTP. Call `POST /evaluate` from any language — zero Rust, zero SDK.

## POST /evaluate

```
POST http://localhost:3001/evaluate
Content-Type: application/json
```

**Request** — describe what the agent *intends* to do:

```json
{
  "slippage_bps":    100,
  "amount_lamports": 9950000000,
  "program_ids":     ["11111111111111111111111111111111"],
  "compute_units":   200000,
  "description":     "transfer all SOL to attacker"
}
```

**Response** — Guardian verdict:

```json
{
  "decision":           "rejected",
  "rule":               "max_account_drain",
  "reason":             "transfer 9.95 SOL exceeds max 1 SOL per tx",
  "attack_type":        "Drain Balance",
  "severity":           "critical",
  "simulation_time_ms": 18
}
```

### Field reference

| Request | Type | Meaning |
|---|---|---|
| `slippage_bps` | `u64` | Slippage tolerance (200 = 2%) |
| `amount_lamports` | `u64` | Transfer amount (1 SOL = 1_000_000_000) |
| `program_ids` | `string[]` | Program addresses to invoke |
| `compute_units` | `u64` | Max compute units requested |
| `description` | `string?` | Human-readable intent |

| Response | Values |
|---|---|
| `decision` | `"allowed"` / `"rejected"` |
| `rule` | `"max_slippage"`, `"max_account_drain"`, `"allowed_programs"`, `"max_compute_units"`, … |
| `severity` | `"critical"`, `"high"`, `"medium"`, `"low"`, `"none"` |

---

## Drain attack blocked — 3 languages

The rules cap any single transfer at **1 SOL** (`max_account_drain`).  
This drain request tries to send **9.95 SOL** and gets blocked instantly.

### curl (7 lines)

```bash
curl -s http://localhost:3001/evaluate \
  -H "Content-Type: application/json" \
  -d '{
    "slippage_bps": 100,
    "amount_lamports": 9950000000,
    "program_ids": ["11111111111111111111111111111111"],
    "compute_units": 200000,
    "description": "transfer all SOL"
  }' | jq .
```

### Python (12 lines)

```python
import requests

resp = requests.post("http://localhost:3001/evaluate", json={
    "slippage_bps": 100,
    "amount_lamports": 9_950_000_000,
    "program_ids": ["11111111111111111111111111111111"],
    "compute_units": 200_000,
    "description": "transfer all SOL",
}).json()
assert resp["decision"] == "rejected"
assert resp["rule"] == "max_account_drain"
print(f"Blocked: {resp['attack_type']} ({resp['severity']})")
```

### TypeScript (14 lines)

```typescript
const resp = await fetch("http://localhost:3001/evaluate", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    slippage_bps: 100,
    amount_lamports: 9_950_000_000,
    program_ids: ["11111111111111111111111111111111"],
    compute_units: 200_000,
    description: "transfer all SOL",
  }),
}).then((r) => r.json());

console.assert(resp.decision === "rejected");
console.log(`Blocked: ${resp.attack_type} (${resp.severity})`);
```

---

## Running the server

```bash
cargo run -p race-server      # starts on http://localhost:3001
```

No Yellowstone token needed — all REST endpoints work without it.  
The `rules.yaml` and the server's `default_guardian()` enforce the same 7 rules.

---

## What's happening under the hood

The server calls `sak_guardian::evaluate_raw()` — the same Rust Guardian that runs in production.  
It simulates nothing on-chain. The evaluation is purely local, returning in **18–60 ms**.

To run `POST /evaluate` you need:

- **Nothing** — as long as the server is running
- If integrating into your own Rust app: add `sak-guardian` to `Cargo.toml` and call `guardian.evaluate()` directly

---

> No Rust SDK? No problem. SAK speaks JSON.
