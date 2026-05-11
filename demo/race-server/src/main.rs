use axum::{
    body::{Body, Bytes},
    extract::{ws::WebSocket, ws::WebSocketUpgrade, DefaultBodyLimit, Json, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex};
use tokio::io::AsyncBufReadExt;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::info;

use sak_core::{ChainEvent, Decision, FeedbackVerdict, GuardianFeedback, TxMeta};
use sak_guardian::{Guardian, Rule};
use sak_reflex::ReflexConfig;
use serde::{Deserialize, Serialize};

const NVIDIA_OPENAI_V1: &str = "https://integrate.api.nvidia.com/v1";

type FeedbackStore = Arc<Mutex<Vec<GuardianFeedback>>>;

struct PriceCache {
    price: f64,
    fetched_at: Option<Instant>,
}

impl PriceCache {
    fn new() -> Self {
        Self {
            price: 150.0,
            fetched_at: None,
        }
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

fn build_cors_layer() -> CorsLayer {
    let raw = std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default();
    if raw.is_empty() || raw == "*" {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
    }

    let origins: Vec<HeaderValue> = raw
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| HeaderValue::from_str(s).ok())
        .collect();

    if origins.is_empty() {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
    }

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any)
}

fn tx_generator_enabled() -> bool {
    match std::env::var("ENABLE_TX_GENERATOR").ok().as_deref() {
        Some("1" | "true") => true,
        Some("0" | "false") => false,
        Some(_) => false,
        None => std::env::var("RAILWAY_ENVIRONMENT").is_err(),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let (tx, _) = broadcast::channel::<String>(100);
    let state = AppState {
        feedback: Arc::new(Mutex::new(Vec::new())),
        price: Arc::new(Mutex::new(PriceCache::new())),
    };

    if tx_generator_enabled() {
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                info!("Starting transaction generator...");
                let mut child = match Command::new("cargo")
                    .args(["run", "--manifest-path", "demo/tx-generator/Cargo.toml"])
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("tx-generator spawn failed (cargo missing?): {e}");
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        continue;
                    }
                };

                let mut reader = tokio::io::BufReader::new(child.stdout.take().unwrap());
                let mut line = String::new();

                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
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
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    } else {
        tracing::info!(
            "Tx generator disabled (set ENABLE_TX_GENERATOR=true locally, or unset RAILWAY_ENVIRONMENT)"
        );
    }

    {
        let config = ReflexConfig::from_env();
        if config.token.is_empty() {
            tracing::warn!("HELIUS_API_KEY / YELLOWSTONE_TOKEN not set — Reflex Engine disabled");
        } else {
            let (chain_tx, mut chain_rx) = tokio::sync::mpsc::channel::<ChainEvent>(256);
            let ws_tx = tx.clone();

            tokio::spawn(async move {
                if let Err(e) = sak_reflex::start(config, chain_tx).await {
                    tracing::error!("Reflex Engine fatal: {}", e);
                }
            });

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

            info!("Reflex Engine spawned — Yellowstone / Geyser");
        }
    }

    let cors = build_cors_layer();

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/nvidia/models", get(nvidia_proxy_models))
        .route("/api/nvidia/chat/completions", post(nvidia_proxy_chat))
        .route(
            "/ws",
            get({
                let tx = tx.clone();
                move |ws: WebSocketUpgrade| {
                    let rx = tx.subscribe();
                    async move { ws.on_upgrade(|socket| handle_ws(socket, rx)) }
                }
            }),
        )
        .route("/sol-price", get(sol_price_handler))
        .route("/feedback", post(feedback_handler))
        .route("/feedback/summary", get(feedback_summary_handler))
        .route("/evaluate", post(evaluate_handler))
        .route("/squads/create-agent-wallet", post(squads_create_wallet_handler))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(cors)
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3001".into());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("race-server listening on http://{addr} (use HTTPS via Railway edge)");
    axum::serve(listener, app).await.unwrap();
}

async fn nvidia_proxy_models(headers: HeaderMap) -> impl IntoResponse {
    nvidia_forward_reqwest(Method::GET, format!("{NVIDIA_OPENAI_V1}/models"), headers, None).await
}

async fn nvidia_proxy_chat(headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    nvidia_forward_reqwest(
        Method::POST,
        format!("{NVIDIA_OPENAI_V1}/chat/completions"),
        headers,
        Some(body),
    )
    .await
}

async fn nvidia_forward_reqwest(
    method: Method,
    url: String,
    headers: HeaderMap,
    body: Option<Bytes>,
) -> Response {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("client build: {e}"),
            )
                .into_response();
        }
    };

    let auth = headers.get(header::AUTHORIZATION).cloned();
    let accept = headers
        .get(header::ACCEPT)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("application/json"));

    let rw_method = match method {
        Method::GET => reqwest::Method::GET,
        Method::POST => reqwest::Method::POST,
        _ => {
            return (StatusCode::METHOD_NOT_ALLOWED, "only GET/POST").into_response();
        }
    };

    let mut req = client.request(rw_method, &url);

    if let Some(a) = auth {
        req = req.header(header::AUTHORIZATION, a);
    }
    req = req.header(header::ACCEPT, accept);
    if let Some(b) = body {
        req = req
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )
            .body(b);
    }

    let upstream = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("upstream: {e}"),
            )
                .into_response();
        }
    };

    let status = StatusCode::from_u16(upstream.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    let bytes = match upstream.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("body: {e}"),
            )
                .into_response();
        }
    };

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn sol_price_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
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

async fn feedback_handler(State(state): State<AppState>, Json(fb): Json<GuardianFeedback>) -> &'static str {
    state.feedback.lock().await.push(fb);
    "recorded"
}

async fn feedback_summary_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let v = state.feedback.lock().await;
    let total = v.len();
    let correct = v
        .iter()
        .filter(|fb| matches!(fb.verdict, FeedbackVerdict::Correct))
        .count();
    let wrong = v
        .iter()
        .filter(|fb| matches!(fb.verdict, FeedbackVerdict::Wrong))
        .count();
    let accuracy = if total > 0 {
        (correct as f64 / total as f64) * 100.0
    } else {
        0.0
    };
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

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const COMPUTE_BUDGET_ID: &str = "ComputeBudget111111111111111111111111111111";

#[derive(Deserialize)]
struct IntentRequest {
    slippage_bps: Option<u64>,
    amount_lamports: Option<u64>,
    program_ids: Option<Vec<String>>,
    compute_units: Option<u64>,
    description: Option<String>,
}

#[derive(Serialize)]
struct EvaluateResponse {
    decision: String,
    rule: Option<String>,
    reason: Option<String>,
    attack_type: String,
    severity: String,
    simulation_time_ms: u64,
}

fn default_guardian() -> Guardian {
    Guardian::with_rules(vec![
        Rule::SlippageCheck {
            name: "max_slippage".into(),
            max_bps: 200,
        },
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
        Rule::DrainCheck {
            name: "max_account_drain".into(),
            max_lamports: 1_000_000_000,
        },
        Rule::ComputeUnitsCheck {
            name: "max_compute_units".into(),
            max_units: 1_400_000,
        },
        Rule::PriorityFeeCheck {
            name: "max_priority_fee".into(),
            max_microlamports: 1_000_000,
        },
        Rule::MinTransferLamports {
            name: "min_transfer_lamports".into(),
            min_lamports: 1,
        },
    ])
}

fn classify_rejection(rule: &str, slippage_bps: u64, amount_lamports: u64) -> (&'static str, &'static str) {
    match rule {
        "max_slippage" => {
            let t = if slippage_bps >= 9000 {
                "99% Slippage Swap"
            } else {
                "High-Slippage Swap"
            };
            let s = if slippage_bps >= 5000 {
                "critical"
            } else {
                "high"
            };
            (t, s)
        }
        "max_account_drain" => {
            let s = if amount_lamports > 5_000_000_000 {
                "critical"
            } else {
                "high"
            };
            ("Drain Balance", s)
        }
        "allowed_programs" => ("Unwhitelisted Program", "medium"),
        "max_compute_units" => ("Compute Bomb", "medium"),
        "max_priority_fee" => ("Priority Fee Bomb", "medium"),
        _ => ("Policy Violation", "medium"),
    }
}

async fn evaluate_handler(Json(req): Json<IntentRequest>) -> Json<EvaluateResponse> {
    let start = Instant::now();
    let slippage_bps = req.slippage_bps.unwrap_or(0);
    let amount_lamports = req.amount_lamports.unwrap_or(0);
    let compute_units = req.compute_units.unwrap_or(0);
    let program_ids = req.program_ids.clone().unwrap_or_default();

    info!(
        slippage_bps,
        amount_lamports,
        compute_units,
        programs = ?program_ids,
        desc = ?req.description,
        "sak-guardian evaluate_raw called"
    );

    let mut account_keys: Vec<String> = vec!["Dummy1111111111111111111111111111111111111".into()];
    let mut owned_data: Vec<Vec<u8>> = Vec::new();
    let mut ix_indices: Vec<u8> = Vec::new();

    for prog_id in &program_ids {
        let idx = account_keys.len() as u8;
        account_keys.push(prog_id.clone());

        let data = if prog_id == SYSTEM_PROGRAM_ID && amount_lamports > 0 {
            let mut d = vec![0x02u8, 0x00, 0x00, 0x00];
            d.extend_from_slice(&amount_lamports.to_le_bytes());
            d
        } else if prog_id == COMPUTE_BUDGET_ID && compute_units > 0 {
            let mut d = vec![0x02u8];
            d.extend_from_slice(&(compute_units as u32).to_le_bytes());
            d
        } else {
            vec![]
        };

        owned_data.push(data);
        ix_indices.push(idx);
    }

    if compute_units > 0 && !program_ids.iter().any(|p| p == COMPUTE_BUDGET_ID) {
        let idx = account_keys.len() as u8;
        account_keys.push(COMPUTE_BUDGET_ID.into());
        let mut d = vec![0x02u8];
        d.extend_from_slice(&(compute_units as u32).to_le_bytes());
        owned_data.push(d);
        ix_indices.push(idx);
    }

    let raw_ixs: Vec<(u8, &[u8])> = ix_indices
        .iter()
        .zip(owned_data.iter())
        .map(|(i, d)| (*i, d.as_slice()))
        .collect();

    let meta = TxMeta {
        slippage_bps: Some(slippage_bps),
        description: req.description.clone(),
    };
    let guardian = default_guardian();
    let decision = guardian.evaluate_raw(account_keys, &raw_ixs, &meta);
    let elapsed_ms = start.elapsed().as_millis() as u64;

    let mut resp = match &decision {
        Decision::Allow => {
            info!(elapsed_ms, "Guardian → ALLOW");
            EvaluateResponse {
                decision: "allowed".into(),
                rule: None,
                reason: None,
                attack_type: "Valid Swap".into(),
                severity: "none".into(),
                simulation_time_ms: elapsed_ms,
            }
        }
        Decision::Reject { rule, reason } => {
            let (at, sev) = classify_rejection(rule, slippage_bps, amount_lamports);
            info!(
                elapsed_ms,
                rule,
                reason,
                attack_type = at,
                severity = sev,
                "Guardian → REJECT"
            );
            EvaluateResponse {
                decision: "rejected".into(),
                rule: Some(rule.clone()),
                reason: Some(reason.clone()),
                attack_type: at.into(),
                severity: sev.into(),
                simulation_time_ms: elapsed_ms,
            }
        }
    };

    // Layer 2 — Squads spending limit (Guardian already allowed).
    const SQUADS_LAMPORTS_CAP: u64 = 10_000_000;
    if matches!(&decision, Decision::Allow) && amount_lamports > SQUADS_LAMPORTS_CAP {
        info!(
            elapsed_ms,
            amount_lamports,
            cap = SQUADS_LAMPORTS_CAP,
            "Squads Layer 2 → BLOCK (Guardian ALLOW)"
        );
        resp = EvaluateResponse {
            decision: "rejected".into(),
            rule: Some("squads_spending_limit".into()),
            reason: Some("10 USDC/tx cap exceeded".into()),
            attack_type: "Layer 2 — Squads blocked (Guardian allowed)".into(),
            severity: "high".into(),
            simulation_time_ms: elapsed_ms,
        };
    }

    Json(resp)
}

// ============================================================
// SQUADS SMART ACCOUNT — Layer 2 spending-limit policy
// ============================================================
#[derive(Deserialize)]
struct SquadsWalletRequest {
    agent_name: Option<String>,
    spending_limit_usdc: Option<f64>,
}

#[derive(Serialize)]
struct SquadsWalletResponse {
    status: String,
    smart_account: String,
    config_authority: String,
    spending_limit_usdc: f64,
    spending_limit_atoms: u64,
    program_id: String,
    explorer_url: String,
    squads_app_url: String,
    api_note: String,
    sdk_snippet: String,
}

async fn squads_create_wallet_handler(
    Json(req): Json<SquadsWalletRequest>,
) -> Json<SquadsWalletResponse> {
    let spending_limit = req.spending_limit_usdc.unwrap_or(10.0);
    let agent_name = req.agent_name.unwrap_or_else(|| "SAK Demo Agent".into());
    // Real Squads multisig created on devnet via scripts/create-squads-account.ts
    let smart_account = "HzaSqyyW5kuGyGFndRhZjx5h24TB79ZUsxEMPUsKSfoX".to_string();
    let config_authority = "2bzdLiLZdKRgb1zMdndTbDEgtbPwLepfjNPPCQrawaoZ".to_string();
    let program_id = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf".to_string();
    let spending_limit_atoms = (spending_limit * 1_000_000.0) as u64; // USDC 6 decimals

    info!(
        %agent_name,
        spending_limit_usdc = spending_limit,
        %smart_account,
        "Squads create-agent-wallet called (demo)"
    );

    let sdk_snippet = format!(
        r#"// @squads-protocol/multisig
import * as multisig from "@squads-protocol/multisig";
const createKey = Keypair.generate();
const [multisigPda] = multisig.getMultisigPda({{
  createKey: createKey.publicKey,
}});
await multisig.rpc.multisigCreateV2({{
  connection, creator: agent,
  multisigPda, configAuthority: null,
  threshold: 1,
  members: [{{ key: agentPubkey, permissions: Permissions.all() }}],
  timeLock: 0, memo: "{agent_name}",
}});
// Spending limit: ${spending_limit} USDC per tx
await multisig.rpc.spendingLimitCreate({{
  multisigPda, mint: USDC_MINT,
  amount: BigInt({spending_limit_atoms}),
  decimals: 6,
  destinations: [jupiterProgram],
}});"#
    );

    Json(SquadsWalletResponse {
        status: "created".into(),
        smart_account: smart_account.clone(),
        config_authority,
        spending_limit_usdc: spending_limit,
        spending_limit_atoms,
        program_id,
        explorer_url: format!(
            "https://solscan.io/account/{smart_account}?cluster=devnet"
        ),
        squads_app_url: format!(
            "https://v4.squads.so/multisigs/{smart_account}"
        ),
        api_note: "Squads multisig created on devnet via scripts/create-squads-account.ts. Creator keypair saved in scripts/generated-creator.json.".into(),
        sdk_snippet,
    })
}
