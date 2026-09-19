// Copyright 2026 Ashton Mozano / mac-sysmon-rs contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use clap::Parser;
use mac_sysmon::collector::MetricsCollector;
use mac_sysmon::server::{self, AppState};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(
    name = "mac-sysmon",
    version = "0.1.0",
    author = "Ashton Mozano",
    about = "macOS System Resource & Process Monitor with Live Streaming and Vanilla TS UI"
)]
pub struct Args {
    /// Host to bind server to
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to listen on
    #[arg(short = 'p', long, default_value_t = 3000)]
    pub port: u16,

    /// Metrics refresh interval in milliseconds
    #[arg(short = 'i', long, default_value_t = 1000)]
    pub interval: u64,

    /// Automatically open browser on launch
    #[arg(long, default_value_t = false)]
    pub open: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Setup structured logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| format!("Failed to set tracing subscriber: {}", e))?;

    info!("Starting macOS System Monitor (mac-sysmon-rs v0.1.0)");
    info!("Target Host: {}, Port: {}, Interval: {}ms", args.host, args.port, args.interval);

    // Initialize collector and broadcast channel
    let (collector, latest_metrics, broadcast_tx) = MetricsCollector::new();

    // Spawn the background metrics collection loop
    let interval_ms = args.interval;
    tokio::spawn(async move {
        collector.run_loop(interval_ms).await;
    });

    // Create Axum application router
    let app_state = AppState {
        latest_metrics,
        broadcast_tx,
    };
    let app = server::create_router(app_state);

    // Bind server
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_url = format!("http://localhost:{}", args.port);
    info!("========================================================");
    info!("🚀 mac-sysmon is live and listening on {}", local_url);
    info!("📊 Real-time WebSocket stream available at ws://localhost:{}/ws", args.port);
    info!("========================================================");

    // Optionally open default browser
    if args.open {
        let url = local_url.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            info!("Opening browser at {}", url);
            if let Err(e) = open::that(&url) {
                error!("Failed to open browser: {}", e);
            }
        });
    }

    // Run server with graceful shutdown on Ctrl+C / SIGINT / SIGTERM
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("mac-sysmon shut down cleanly.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, initiating graceful shutdown...");
        },
        _ = terminate => {
            info!("Received SIGTERM, initiating graceful shutdown...");
        },
    }
}
