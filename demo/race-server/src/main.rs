use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade}, Json, State},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use tokio::sync::{broadcast, Mutex};
use tokio::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use tracing::info;
use sak_core::{GuardianFeedback, FeedbackVerdict};

/// Shared feedback store.
type FeedbackStore = Arc<Mutex<Vec<GuardianFeedback>>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (tx, _) = broadcast::channel::<String>(100);
    let feedback_store: FeedbackStore = Arc::new(Mutex::new(Vec::new()));

    // Spawn transaction generator as a subprocess
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
                    Ok(0) => break, // EOF
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

    let app = Router::new()
        .route("/ws", get(move |ws: WebSocketUpgrade| {
            let rx = tx.subscribe();
            async move { ws.on_upgrade(|socket| handle_ws(socket, rx)) }
        }))
        .route("/feedback", post({
            let store = feedback_store.clone();
            move |Json(fb): Json<GuardianFeedback>| async move {
                let mut v = store.lock().await;
                v.push(fb);
                "recorded"
            }
        }))
        .route("/feedback/summary", get({
            let store = feedback_store.clone();
            move || async move {
                let v = store.lock().await;
                let total = v.len();
                let correct = v.iter().filter(|fb| matches!(fb.verdict, FeedbackVerdict::Correct)).count();
                let wrong = v.iter().filter(|fb| matches!(fb.verdict, FeedbackVerdict::Wrong)).count();
                let accuracy = if total > 0 {
                    (correct as f64 / total as f64) * 100.0
                } else { 0.0 };
                Json(serde_json::json!({
                    "total": total,
                    "correct": correct,
                    "wrong": wrong,
                    "accuracy": accuracy,
                }))
            }
        }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    info!("WebSocket server running on ws://localhost:3001");
    axum::serve(listener, app.with_state(feedback_store)).await.unwrap();
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
