# SAK API Reference

Three surfaces:

| Document | Audience | What it covers |
|---|---|---|
| [sak-sdk](sak-sdk.md) | Agent developers | High-level `Kernel` API — submit transactions, manage state, subscribe to chain events |
| [sak-guardian](sak-guardian.md) | Protocol integrators | `Guardian` struct — rule configuration, evaluation (simulated + raw), `Rule` YAML schema |
| [race-server](race-server.md) | Demo operators | HTTP + WebSocket endpoints — evaluate intents, proxy NVIDIA, SOL price, Squads policy |

## Interactive Spec

An OpenAPI 3.0 specification covering the race-server's HTTP endpoints is at:
[`demo/race-server/openapi.yaml`](../demo/race-server/openapi.yaml)

Open it in [Swagger Editor](https://editor.swagger.io/) or your IDE's OpenAPI viewer for interactive exploration.

## Quick Links

- **Rust SDK docs** — See [sak-sdk.md](sak-sdk.md) for `Kernel::submit()`, `KernelConfig`, builder pattern
- **Guardian rules** — See [sak-guardian.md](sak-guardian.md) for `Rule` YAML schema, `evaluate()` vs `evaluate_raw()`, `TxView`, `SimulationResult`
- **REST API** — See [race-server.md](race-server.md) for HTTP endpoints, WebSocket stream, environment variables
- **OpenAPI spec** — Open `demo/race-server/openapi.yaml` in an OpenAPI viewer for interactive docs with request/response examples

## What to Use When

| You want to... | Use this |
|---|---|
| Integrate SAK into your Rust agent | `sak-sdk::Kernel` — see [sak-sdk.md](sak-sdk.md) |
| Customize Guardian rules | `sak-guardian::Guardian` + `rules.yaml` — see [sak-guardian.md](sak-guardian.md) |
| Test transactions against Guardian from a browser | `POST /evaluate` — see [race-server.md](race-server.md) |
| Stream live Guardian decisions | `GET /ws` (WebSocket) — see [race-server.md](race-server.md) |
| Call Guardian from TypeScript/Python | Use the HTTP examples in `sak-docs.json` (ts_example, py_example) |
| Deploy the demo server | See [race-server.md](race-server.md) → Environment Variables |
