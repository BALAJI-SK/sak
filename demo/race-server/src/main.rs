use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use tokio::sync::broadcast;
use tokio::process::Command;
use std::process::Stdio;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (tx, _) = broadcast::channel::<String>(100);

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
