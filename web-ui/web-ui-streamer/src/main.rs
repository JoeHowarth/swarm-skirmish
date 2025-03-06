//! Example websocket server.
//!
//! Run the server with
//! ```not_rust
//! cargo run -p example-websockets --bin example-websockets
//! ```
//!
//! Run a browser client with
//! ```not_rust
//! firefox http://localhost:3000
//! ```
//!
//! Alternatively you can run the rust client (showing two
//! concurrent websocket connections being established) with
//! ```not_rust
//! cargo run -p example-websockets --bin example-client
//! ```
#![allow(unused_imports)]

use std::{net::SocketAddr, ops::ControlFlow, path::PathBuf};

// allows to extract the IP of connecting user
use axum::extract::connect_info::ConnectInfo;
use axum::{
    body::Bytes,
    extract::ws::{
        CloseFrame,
        Message,
        Utf8Bytes,
        WebSocket,
        WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::any,
    Router,
};
use axum_extra::{headers, TypedHeader};
use eyre::Result;
// allows to split the websocket stream into separate TX and RX branches
use futures::{sink::SinkExt, stream::StreamExt};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::debug;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod axum_server;
mod journal_ingestor;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    setup_logging();

    let (rx, map) = journal_ingestor::journal_streamer();

    tokio::spawn(axum_server::run_axum_server(rx, map)).await?
}

fn setup_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    format!(
                        "{}=debug,tower_http=debug",
                        env!("CARGO_CRATE_NAME")
                    )
                    .into()
                }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
