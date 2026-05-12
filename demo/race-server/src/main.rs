use axum::{
    body::{Body, Bytes},
    extract::{ws::WebSocket, ws::WebSocketUpgrade, DefaultBodyLimit, Json, State},
    http::{header, request::Parts, HeaderMap, HeaderValue, Method, StatusCode},
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
use sak_covalent::CovalentClient;
use sak_jito::JitoClient;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    /// Loaded once at startup. `Guardian::evaluate_raw` is `&self`, so an
    /// `Arc` is sufficient — no per-request mutex required.
    guardian: Arc<Guardian>,
    /// Covalent GoldRush client for token verification and wallet analysis.
    covalent: Option<Arc<CovalentClient>>,
    /// Jito client for MEV-protected bundle submission.
    jito: Arc<JitoClient>,
}

fn sak_demo_pages_origin(origin: &str) -> bool {
    origin == "https://sak-devnet-test.pages.dev"
        || origin.ends_with(".sak-devnet-test.pages.dev")
        || origin == "https://sak-d89.pages.dev"
        || origin.ends_with(".sak-d89.pages.dev")
        || origin == "https://balaji-sk.github.io"
}

fn localhost_dev_origin(origin: &str) -> bool {
    origin.starts_with("http://localhost:") || origin.starts_with("http://127.0.0.1:")
}

fn build_cors_layer() -> CorsLayer {
    let raw = std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default();
    if raw.is_empty() || raw == "*" {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
    }

    let explicit: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if explicit.is_empty() {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
    }

    let explicit = Arc::new(explicit);
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate({
            let explicit = Arc::clone(&explicit);
            move |origin: &HeaderValue, _parts: &Parts| {
                let Ok(s) = origin.to_str() else {
                    return false;
                };
                if explicit.iter().any(|e| e == s) {
                    return true;
                }
                sak_demo_pages_origin(s) || localhost_dev_origin(s)
            }
        }))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
}

/// Resolve the directory containing rule pack YAML files.
///
/// Search order:
///   1. `RULE_PACKS_DIR` environment variable (absolute or relative to cwd).
///   2. `./packs` relative to the current working directory.
///
/// Deliberately does *not* fall back to the build-host workspace
/// (`CARGO_MANIFEST_DIR`) — that path doesn't exist in production
/// containers and would silently load nothing. When no filesystem
/// packs are found, callers fall back to `EMBEDDED_PACKS` instead.
fn rule_packs_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RULE_PACKS_DIR") {
        let path = PathBuf::from(p);
        if path.is_dir() {
            return Some(path);
        }
    }
    let cwd = PathBuf::from("packs");
    if cwd.is_dir() {
        return Some(cwd);
    }
    None
}

/// Rule packs compiled into the binary so the deployed service is
/// self-contained — Railway / Fly / GitHub Actions runners can serve
/// the right rule count without copying YAML files around.
///
/// Filesystem packs (under `./packs/` or `RULE_PACKS_DIR`) take precedence
/// when they exist so local edits to YAML are picked up by a simple restart.
const EMBEDDED_PACKS: &[(&str, &str)] = &[
    ("defaults.yaml",          include_str!("../../../packs/defaults.yaml")),
    ("solana-core.yaml",       include_str!("../../../packs/solana-core.yaml")),
    ("exploits-blocklist.yaml", include_str!("../../../packs/exploits-blocklist.yaml")),
    ("tokens-blocklist.yaml",  include_str!("../../../packs/tokens-blocklist.yaml")),
];

/// Build the runtime Guardian.
///
/// Load order:
///   1. `*.yaml` from the packs directory (if it exists and is non-empty).
///   2. Embedded packs compiled into the binary via `include_str!`.
///   3. Hand-written `default_guardian()` as a last-resort safety net.
fn load_runtime_guardian() -> Guardian {
    if let Some(dir) = rule_packs_dir() {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("yaml"))
            .collect();
        paths.sort();
        if !paths.is_empty() {
            match Guardian::from_yaml_files(&paths) {
                Ok(g) => {
                    let s = g.stats();
                    info!(
                        total = s.total,
                        packs = s.packs.len(),
                        source = "filesystem",
                        "Guardian loaded from rule packs"
                    );
                    return g;
                }
                Err(e) => {
                    tracing::error!(?e, "failed to parse filesystem packs; falling back to embedded");
                }
            }
        } else {
            tracing::warn!(?dir, "packs dir empty; falling back to embedded");
        }
    }

    match Guardian::from_yaml_strings(EMBEDDED_PACKS) {
        Ok(g) => {
            let s = g.stats();
            info!(
                total = s.total,
                packs = s.packs.len(),
                source = "embedded",
                "Guardian loaded from embedded rule packs"
            );
            g
        }
        Err(e) => {
            tracing::error!(?e, "failed to load embedded packs; using hand-written defaults");
            default_guardian()
        }
    }
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

    let (tx, _) = broadcast::channel::<String>(1024);
    let guardian = Arc::new(load_runtime_guardian());

    // Initialize Covalent GoldRush client (optional — requires COVALENT_API_KEY)
    let covalent = CovalentClient::from_env().map(|c| {
        info!("Covalent GoldRush API enabled");
        Arc::new(c)
    });

    // Initialize Jito client for MEV-protected bundle submission
    let jito = Arc::new(JitoClient::from_env());
    info!(
        tip_lamports = jito.tip_lamports(),
        tip_sol = jito.tip_sol(),
        "Jito bundle client initialized"
    );

    let state = AppState {
        feedback: Arc::new(Mutex::new(Vec::new())),
        price: Arc::new(Mutex::new(PriceCache::new())),
        guardian,
        covalent,
        jito,
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
        .route("/rules/stats", get(rules_stats_handler))
        // Covalent GoldRush endpoints
        .route("/covalent/verify-token", post(covalent_verify_token))
        .route("/covalent/token-balances", post(covalent_token_balances))
        .route("/covalent/wallet-risk", post(covalent_wallet_risk))
        .route("/covalent/token-metadata", post(covalent_token_metadata))
        // Jito bundle endpoints
        .route("/jito/submit-bundle", post(jito_submit_bundle))
        .route("/jito/status/:bundle_id", get(jito_bundle_status))
        .route("/jito/info", get(jito_info))

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
        Rule::AccountCountCheck {
            name: "max_accounts".into(),
            max_count: 20,
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
        "max_accounts" => ("Account Count Exceeded", "medium"),
        _ => ("Policy Violation", "medium"),
    }
}

async fn rules_stats_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let stats = state.guardian.stats();
    let by_kind: serde_json::Map<String, serde_json::Value> = stats
        .by_kind
        .iter()
        .map(|(k, v)| (k.to_string(), serde_json::Value::from(*v)))
        .collect();
    let packs: Vec<String> = stats
        .packs
        .iter()
        .map(|p| {
            // Surface just the file name to clients — full paths are noise.
            std::path::Path::new(p)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| p.clone())
        })
        .collect();
    Json(serde_json::json!({
        "total": stats.total,
        "by_kind": by_kind,
        "packs": packs,
    }))
}

async fn evaluate_handler(
    State(state): State<AppState>,
    Json(req): Json<IntentRequest>,
) -> Json<EvaluateResponse> {
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
    let decision = state.guardian.evaluate_raw(account_keys, &raw_ixs, &meta);
    let elapsed_ms = start.elapsed().as_millis() as u64;

    let resp = match &decision {
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

    Json(resp)
}

// ============================================================
// COVALENT GOLDRUSH HANDLERS
// ============================================================

#[derive(Deserialize)]
struct CovalentTokenRequest {
    contract_address: String,
}

#[derive(Deserialize)]
struct CovalentWalletRequest {
    address: String,
}

async fn covalent_verify_token(
    State(state): State<AppState>,
    Json(req): Json<CovalentTokenRequest>,
) -> Json<serde_json::Value> {
    let Some(covalent) = &state.covalent else {
        return Json(serde_json::json!({
            "error": "Covalent API not configured. Set COVALENT_API_KEY env var.",
            "configured": false
        }));
    };

    match covalent.is_token_verified(&req.contract_address).await {
        Ok(verified) => Json(serde_json::json!({
            "contract_address": req.contract_address,
            "verified": verified,
            "source": "covalent_goldrush"
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Covalent API error: {}", e),
            "contract_address": req.contract_address
        })),
    }
}

async fn covalent_token_balances(
    State(state): State<AppState>,
    Json(req): Json<CovalentWalletRequest>,
) -> Json<serde_json::Value> {
    let Some(covalent) = &state.covalent else {
        return Json(serde_json::json!({
            "error": "Covalent API not configured. Set COVALENT_API_KEY env var.",
            "configured": false
        }));
    };

    match covalent.get_token_balances(&req.address).await {
        Ok(balances) => Json(serde_json::json!({
            "address": req.address,
            "chain": "solana-mainnet",
            "balances": balances,
            "count": balances.len(),
            "source": "covalent_goldrush"
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Covalent API error: {}", e),
            "address": req.address
        })),
    }
}

async fn covalent_wallet_risk(
    State(state): State<AppState>,
    Json(req): Json<CovalentWalletRequest>,
) -> Json<serde_json::Value> {
    let Some(covalent) = &state.covalent else {
        return Json(serde_json::json!({
            "error": "Covalent API not configured. Set COVALENT_API_KEY env var.",
            "configured": false
        }));
    };

    match covalent.assess_wallet_risk(&req.address).await {
        Ok(assessment) => Json(serde_json::json!({
            "assessment": assessment,
            "source": "covalent_goldrush"
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Covalent API error: {}", e),
            "address": req.address
        })),
    }
}

async fn covalent_token_metadata(
    State(state): State<AppState>,
    Json(req): Json<CovalentTokenRequest>,
) -> Json<serde_json::Value> {
    let Some(covalent) = &state.covalent else {
        return Json(serde_json::json!({
            "error": "Covalent API not configured. Set COVALENT_API_KEY env var.",
            "configured": false
        }));
    };

    match covalent.get_token_metadata(&req.contract_address).await {
        Ok(metadata) => Json(serde_json::json!({
            "metadata": metadata,
            "source": "covalent_goldrush"
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Covalent API error: {}", e),
            "contract_address": req.contract_address
        })),
    }
}

// ============================================================
// JITO BUNDLE HANDLERS
// ============================================================

#[derive(Deserialize)]
struct JitoBundleRequest {
    transactions: Vec<String>,
}

async fn jito_submit_bundle(
    State(state): State<AppState>,
    Json(req): Json<JitoBundleRequest>,
) -> Json<serde_json::Value> {
    info!(
        tx_count = req.transactions.len(),
        "Jito bundle submission requested"
    );

    match state.jito.submit_bundle(req.transactions).await {
        Ok(result) => Json(serde_json::json!({
            "result": result,
            "source": "jito_block_engine"
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Jito submission error: {}", e)
        })),
    }
}

async fn jito_bundle_status(
    State(state): State<AppState>,
    axum::extract::Path(bundle_id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    match state.jito.get_bundle_status(&bundle_id).await {
        Ok(status) => Json(serde_json::json!({
            "status": status,
            "source": "jito_block_engine"
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("Jito status error: {}", e),
            "bundle_id": bundle_id
        })),
    }
}

async fn jito_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "tip_lamports": state.jito.tip_lamports(),
        "tip_sol": state.jito.tip_sol(),
        "tip_account": state.jito.tip_account(),
        "block_engine": "https://mainnet.block-engine.jito.wtf",
        "source": "jito"
    }))
}

