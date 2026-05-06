use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    routing::get,
    Router,
};
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use tracing::{info, error};

use sak_guardian::Guardian;
use sak_core::{Decision, TxMeta};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use solana_signer::Signer;
use solana_address::Address;
use rand::Rng;

struct TxFactory {
    svm: LiteSVM,
    payer: Keypair,
}

impl TxFactory {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap_or_default();
        Self { svm, payer }
    }

    fn generate(&self) -> (Transaction, TxMeta) {
        let mut rng = rand::thread_rng();
        let recipient = Address::new_unique();

        if rng.gen_bool(0.7) {
            // Evil transaction (70%)
            let (lamports, slippage_bps, desc) = match rng.gen_range(0..5) {
                0 => (u64::MAX, None, "Drain entire balance"),
                1 => (1_000, Some(9900), "99% slippage swap"),
                2 => (5_000_000_000, None, "Disguised fee drain"),
                _ => (1_000, None, "Unknown program"),
            };

            let ix = transfer(&self.payer.pubkey(), &recipient, lamports);
            let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
            let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
            let meta = TxMeta {
                slippage_bps,
                description: Some(desc.into()),
                ..Default::default()
            };
            (tx, meta)
        } else {
            // Valid transaction (30%)
            let ix = transfer(&self.payer.pubkey(), &recipient, 500_000);
            let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
            let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
            let meta = TxMeta {
                slippage_bps: Some(100),
                description: Some("Valid transfer 0.005 SOL".into()),
                ..Default::default()
            };
            (tx, meta)
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (tx, _) = broadcast::channel::<String>(100);
    let tx_clone = tx.clone();

    // Spawn transaction generator task
    tokio::spawn(async move {
        info!("Transaction generator started");
        let factory = TxFactory::new();
        let mut guardian = match Guardian::from_yaml("rules.yaml") {
            Ok(g) => g,
            Err(e) => {
                error!("Failed to load rules.yaml: {}", e);
                return;
            }
        };
        let mut ticker = interval(Duration::from_secs(2));

        loop {
            ticker.tick().await;

            let (transaction, meta) = factory.generate();
            let decision = guardian.evaluate(&transaction.into());

            let log_entry = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "decision": match &decision {
                    Decision::Allow => "allowed",
                    Decision::Reject { .. } => "rejected",
                },
                "rule": match &decision {
                    Decision::Reject { rule, .. } => Some(rule.as_str()),
                    _ => None,
                },
                "reason": match &decision {
                    Decision::Reject { reason, .. } => Some(reason.as_str()),
                    _ => None,
                },
                "description": meta.description,
            });

            let json_str = serde_json::to_string(&log_entry).unwrap();
            info!("Generated: {}", json_str);
            let _ = tx_clone.send(json_str);
        }
    });

    let app = Router::new()
        .route("/ws", get(move |ws: WebSocketUpgrade| {
            let rx = tx.subscribe();
            async move { ws.on_upgrade(|socket| handle_ws(socket, rx)) }
        }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    info!("WebSocket server running on ws://localhost:3001");
    axum::serve(listener, app).await.unwrap();
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
