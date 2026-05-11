use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade}, Json, State},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex};
use tokio::process::Command;
use std::process::Stdio;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use sak_core::{ChainEvent, GuardianFeedback, FeedbackVerdict, Decision, TxMeta};
use sak_guardian::{Guardian, Rule};
use sak_reflex::ReflexConfig;
use serde::{Deserialize, Serialize};

type FeedbackStore = Arc<Mutex<Vec<GuardianFeedback>>>;

struct PriceCache {
    price: f64,
    fetched_at: Option<Instant>,
}

impl PriceCache {
    fn new() -> Self {
        Self { price: 150.0, fetched_at: None }
    }

    fn is_stale(&self) -> bool {
        self.fetched_at
            .map(|t| t.elapsed() > Duration::from_secs(60))
            .unwrap_or(true)
    }
}

type SharedPriceCache = Arc<Mutex<PriceCache>>;

#[derive(Clone)]
struct AppState {
    feedback: FeedbackStore,
    price: SharedPriceCache,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    let (tx, _) = broadcast::channel::<String>(100);
    let state = AppState {
        feedback: Arc::new(Mutex::new(Vec::new())),
        price: Arc::new(Mutex::new(PriceCache::new())),
    };

    // Spawn transaction generator subprocess
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        loop {
            info!("Starting transaction generator...");
            let mut child = Command::new("cargo")
                .args(&["run", "--manifest-path", "demo/tx-generator/Cargo.toml"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start tx-generator");

            let mut reader = tokio::io::BufReader::new(child.stdout.take().unwrap());
            let mut line = String::new();

            loop {
                line.clear();
                match tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let _ = tx_clone.send(trimmed.to_string());
                        }
                    }
                    Err(_) => break,
                }
            }

            let _ = child.wait().await;
            info!("Transaction generator exited, restarting in 5s...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    // Spawn Reflex Engine — only if YELLOWSTONE_TOKEN is set, never panic if missing
    {
        let token = std::env::var("YELLOWSTONE_TOKEN").unwrap_or_default();
        if token.is_empty() {
            tracing::warn!("YELLOWSTONE_TOKEN not set — Yellowstone Reflex Engine disabled");
        } else {
            let config = ReflexConfig::from_env();
            let (chain_tx, mut chain_rx) = tokio::sync::mpsc::channel::<ChainEvent>(256);
            let ws_tx = tx.clone();

            // gRPC subscriber — reconnects automatically on error
            tokio::spawn(async move {
                if let Err(e) = sak_reflex::start(config, chain_tx).await {
                    tracing::error!("Reflex Engine fatal: {}", e);
                }
            });

            // Forward SlotUpdate events to the WebSocket broadcast channel
            tokio::spawn(async move {
                while let Some(event) = chain_rx.recv().await {
                    if let ChainEvent::SlotUpdate { slot, .. } = event {
                        let msg = serde_json::json!({
                            "type": "slot_update",
                            "slot": slot,
                        })
                        .to_string();
                        let _ = ws_tx.send(msg);
                    }
                }
                tracing::warn!("Reflex Engine channel closed");
            });

            info!("Reflex Engine spawned — streaming devnet slots via Yellowstone");
        }
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/ws", get({
            let tx = tx.clone();
            move |ws: WebSocketUpgrade| {
                let rx = tx.subscribe();
                async move { ws.on_upgrade(|socket| handle_ws(socket, rx)) }
            }
        }))
        .route("/sol-price", get(sol_price_handler))
        .route("/feedback", post(feedback_handler))
        .route("/feedback/summary", get(feedback_summary_handler))
        .route("/evaluate", post(evaluate_handler))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    info!("WebSocket server running on ws://localhost:3001");
    axum::serve(listener, app).await.unwrap();
}

async fn sol_price_handler(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let mut cache = state.price.lock().await;

    if cache.is_stale() {
        match fetch_sol_price_from_coingecko().await {
            Ok(p) => {
                cache.price = p;
                cache.fetched_at = Some(Instant::now());
            }
            Err(e) => {
                tracing::warn!("Failed to fetch SOL price: {}", e);
            }
        }
    }

    Json(serde_json::json!({ "usd": cache.price }))
}

async fn fetch_sol_price_from_coingecko() -> anyhow::Result<f64> {
    let url = "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd";
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let data: serde_json::Value = client.get(url).send().await?.json().await?;
    let price = data["solana"]["usd"]
        .as_f64()
        .ok_or_else(|| anyhow::anyhow!("missing price field"))?;
    Ok(price)
}

async fn feedback_handler(
    State(state): State<AppState>,
    Json(fb): Json<GuardianFeedback>,
) -> &'static str {
    state.feedback.lock().await.push(fb);
    "recorded"
}

async fn feedback_summary_handler(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let v = state.feedback.lock().await;
    let total = v.len();
    let correct = v.iter().filter(|fb| matches!(fb.verdict, FeedbackVerdict::Correct)).count();
    let wrong = v.iter().filter(|fb| matches!(fb.verdict, FeedbackVerdict::Wrong)).count();
    let accuracy = if total > 0 { (correct as f64 / total as f64) * 100.0 } else { 0.0 };
    Json(serde_json::json!({
        "total": total,
        "correct": correct,
        "wrong": wrong,
        "accuracy": accuracy,
    }))
}

async fn handle_ws(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    while let Ok(msg) = rx.recv().await {
        if socket
            .send(axum::extract::ws::Message::Text(msg))
            .await
            .is_err()
        {
            break;
        }
    }
}

// ── Guardian /evaluate ────────────────────────────────────────────────────────

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const COMPUTE_BUDGET_ID: &str = "ComputeBudget111111111111111111111111111111";

#[derive(Deserialize)]
struct IntentRequest {
    slippage_bps:    Option<u64>,
    amount_lamports: Option<u64>,
    program_ids:     Option<Vec<String>>,
    compute_units:   Option<u64>,
    description:     Option<String>,
}

#[derive(Serialize)]
struct EvaluateResponse {
    decision:            String,
    rule:                Option<String>,
    reason:              Option<String>,
    attack_type:         String,
    severity:            String,
    simulation_time_ms:  u64,
}

fn default_guardian() -> Guardian {
    Guardian::with_rules(vec![
        Rule::SlippageCheck { name: "max_slippage".into(), max_bps: 200 },
        Rule::ProgramWhitelist {
            name: "allowed_programs".into(),
            programs: vec![
                "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4".into(),
                "JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB".into(),
                "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzM3Mh8rh7o".into(),
                "9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP".into(),
                "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".into(),
                "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".into(),
                "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bLT".into(),
                "11111111111111111111111111111111".into(),
                "ComputeBudget111111111111111111111111111111".into(),
                "SysvarRent111111111111111111111111111111111".into(),
            ],
        },
        Rule::DrainCheck        { name: "max_account_drain".into(),  max_lamports: 1_000_000_000 },
        Rule::ComputeUnitsCheck { name: "max_compute_units".into(),  max_units: 1_400_000 },
        Rule::PriorityFeeCheck  { name: "max_priority_fee".into(),   max_microlamports: 1_000_000 },
        Rule::MinTransferLamports { name: "min_transfer_lamports".into(), min_lamports: 1 },
    ])
}

fn classify_rejection(rule: &str, slippage_bps: u64, amount_lamports: u64) -> (&'static str, &'static str) {
    match rule {
        "max_slippage" => {
            let t = if slippage_bps >= 9000 { "99% Slippage Swap" } else { "High-Slippage Swap" };
            let s = if slippage_bps >= 5000 { "critical" } else { "high" };
            (t, s)
        }
        "max_account_drain" => {
            let s = if amount_lamports > 5_000_000_000 { "critical" } else { "high" };
            ("Drain Balance", s)
        }
        "allowed_programs"  => ("Unwhitelisted Program", "medium"),
        "max_compute_units" => ("Compute Bomb",          "medium"),
        "max_priority_fee"  => ("Priority Fee Bomb",     "medium"),
        _                   => ("Policy Violation",      "medium"),
    }
}

async fn evaluate_handler(Json(req): Json<IntentRequest>) -> Json<EvaluateResponse> {
    let start           = Instant::now();
    let slippage_bps    = req.slippage_bps.unwrap_or(0);
    let amount_lamports = req.amount_lamports.unwrap_or(0);
    let compute_units   = req.compute_units.unwrap_or(0);
    let program_ids     = req.program_ids.clone().unwrap_or_default();

    info!(
        slippage_bps,
        amount_lamports,
        compute_units,
        programs = ?program_ids,
        desc = ?req.description,
        "sak-guardian evaluate_raw called"
    );

    // Build account_keys and per-instruction encoded data from the intent.
    // The Guardian's evaluate_raw checks rules against (program_id, instruction_data) pairs.
    let mut account_keys: Vec<String> = vec!["Dummy1111111111111111111111111111111111111".into()];
    let mut owned_data:   Vec<Vec<u8>> = Vec::new();
    let mut ix_indices:   Vec<u8>      = Vec::new();

    for prog_id in &program_ids {
        let idx = account_keys.len() as u8;
        account_keys.push(prog_id.clone());

        let data = if prog_id == SYSTEM_PROGRAM_ID && amount_lamports > 0 {
            // System Transfer discriminant=2 (4-byte LE) + lamports (8-byte LE)
            let mut d = vec![0x02u8, 0x00, 0x00, 0x00];
            d.extend_from_slice(&amount_lamports.to_le_bytes());
            d
        } else if prog_id == COMPUTE_BUDGET_ID && compute_units > 0 {
            // SetComputeUnitLimit: [0x02, units_le_u32]
            let mut d = vec![0x02u8];
            d.extend_from_slice(&(compute_units as u32).to_le_bytes());
            d
        } else {
            vec![]
        };

        owned_data.push(data);
        ix_indices.push(idx);
    }

    // If compute_units declared but ComputeBudget not in program_ids, inject it so the
    // ComputeUnitsCheck rule can fire.
    if compute_units > 0 && !program_ids.iter().any(|p| p == COMPUTE_BUDGET_ID) {
        let idx = account_keys.len() as u8;
        account_keys.push(COMPUTE_BUDGET_ID.into());
        let mut d = vec![0x02u8];
        d.extend_from_slice(&(compute_units as u32).to_le_bytes());
        owned_data.push(d);
        ix_indices.push(idx);
    }

    // Build &[(u8, &[u8])] — borrows owned_data which is still live
    let raw_ixs: Vec<(u8, &[u8])> = ix_indices.iter()
        .zip(owned_data.iter())
        .map(|(i, d)| (*i, d.as_slice()))
        .collect();

    let meta = TxMeta { slippage_bps: Some(slippage_bps), description: req.description.clone() };
    let guardian = default_guardian();
    let decision = guardian.evaluate_raw(account_keys, &raw_ixs, &meta);
    let elapsed_ms = start.elapsed().as_millis() as u64;

    let resp = match &decision {
        Decision::Allow => {
            info!(elapsed_ms, "Guardian → ALLOW");
            EvaluateResponse {
                decision:           "allowed".into(),
                rule:               None,
                reason:             None,
                attack_type:        "Valid Swap".into(),
                severity:           "none".into(),
                simulation_time_ms: elapsed_ms,
            }
        }
        Decision::Reject { rule, reason } => {
            let (at, sev) = classify_rejection(rule, slippage_bps, amount_lamports);
            info!(elapsed_ms, rule, reason, attack_type = at, severity = sev, "Guardian → REJECT");
            EvaluateResponse {
                decision:           "rejected".into(),
                rule:               Some(rule.clone()),
                reason:             Some(reason.clone()),
                attack_type:        at.into(),
                severity:           sev.into(),
                simulation_time_ms: elapsed_ms,
            }
        }
    };

    Json(resp)
}
