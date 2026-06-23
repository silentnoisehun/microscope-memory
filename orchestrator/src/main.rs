// main.rs — Orchestrator Entry Point
mod commands;
mod dispatcher;
mod modules;

use dispatcher::SpineDispatcher;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  🚀 ORA SPINE ORCHESTRATOR ACTIVATED");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let spine_path = "E:/ORA_UNIFIED/ora_spine.mmap";
    let ws_addr = "127.0.0.1:8081";
    let backend_ws_addr = "ws://127.0.0.1:8080/ws";

    // Initialize the Spine Dispatcher
    let dispatcher = std::sync::Arc::new(SpineDispatcher::new(spine_path).await?);
    println!("[✓] Connected to Spine: {}", spine_path);

    // Start the WebSocket Server
    let listener = TcpListener::bind(ws_addr).await?;
    println!("[✓] WebSocket Server listening on: {}", ws_addr);
    println!("[✓] Proxying to Backend: {}", backend_ws_addr);

    while let Ok((stream, _)) = listener.accept().await {
        let dispatcher = dispatcher.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, dispatcher, backend_ws_addr).await {
                eprintln!("[!] Connection error: {}", e);
            }
        });
    }

    Ok(())
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    dispatcher: std::sync::Arc<SpineDispatcher>,
    backend_ws_addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Connect to the real backend (microscope-mem)
    let backend_ws = tokio_tungstenite::connect_async(backend_ws_addr).await?.0;
    let (mut backend_sender, mut backend_receiver) = backend_ws.split();

    println!("[WS] New client connected. Proxying...");

    // Task 1: Client $\rightarrow$ Dispatcher $\rightarrow$ Backend
    let d_clone = dispatcher.clone();
    let client_to_backend = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if msg.is_text() {
                let text = msg.to_text().unwrap_or("");
                
                // Check if it's a command (e.g., starts with '/')
                if text.starts_with('/') {
                    println!("[Dispatcher] Command detected: {}", text);
                    if let Err(e) = d_clone.dispatch_intent(text).await {
                        eprintln!("[!] Dispatch error: {}", e);
                    }
                } else {
                    // Regular message $\rightarrow$ forward to backend
                    let _ = backend_sender.send(msg).await;
                }
            }
        }
    });

    // Task 2: Backend $\rightarrow$ Client
    let backend_to_client = tokio::spawn(async move {
        while let Some(Ok(msg)) = backend_receiver.next().await {
            let _ = ws_sender.send(msg).await;
        }
    });

    // Wait for either to finish
    tokio::select! {
        _ = client_to_backend => println!("[WS] Client disconnected"),
        _ = backend_to_client => println!("[WS] Backend disconnected"),
    }

    Ok(())
}
