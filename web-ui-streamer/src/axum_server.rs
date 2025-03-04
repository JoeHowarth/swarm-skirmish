use std::{
    collections::HashMap,
    net::SocketAddr,
    ops::ControlFlow,
    path::PathBuf,
    sync::Arc,
};

use async_channel::Receiver;
// allows to extract the IP of connecting user
use axum::extract::connect_info::ConnectInfo;
use axum::{
    body::Bytes,
    extract::{
        ws::{CloseFrame, Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::any,
    Json,
    Router,
};
use axum_extra::{headers, TypedHeader};
use dashmap::DashMap;
use eyre::Result;
// allows to split the websocket stream into separate TX and RX branches
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::to_string;
use swarm_lib::JournalEntry;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Define a struct to hold our application state
struct AppState {
    journal_rx: Receiver<JournalEntry>,
    bot_id_to_journals: Arc<DashMap<u32, Vec<JournalEntry>>>,
}

pub async fn run_axum_server(
    journal_rx: Receiver<JournalEntry>,
    bot_id_to_journals: Arc<DashMap<u32, Vec<JournalEntry>>>,
) -> Result<()> {
    let assets_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets_web");

    // Create our application state
    let state = Arc::new(AppState {
        journal_rx,
        bot_id_to_journals,
    });

    // Configure CORS
    let cors = CorsLayer::new()
        // Allow requests from any origin
        .allow_origin(Any)
        // Allow common HTTP methods
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        // Allow common headers
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
        ]);
        // Allow credentials (cookies, etc.)
        // .allow_credentials(true);

    // build our application with some routes
    let app = Router::new()
        .fallback_service(
            ServeDir::new(assets_dir).append_index_html_on_directories(true),
        )
        .route("/journals", axum::routing::get(get_journals))
        .route("/ws", any(ws_handler))
        // Add our state to the router
        .with_state(state)
        // Add CORS layer
        .layer(cors)
        // logging so we can see whats going on
        .layer(
            TraceLayer::new_for_http().make_span_with(
                DefaultMakeSpan::default().include_headers(true),
            ),
        );

    // run it with hyper
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(Into::into)
}

async fn get_journals(
    State(state): State<Arc<AppState>>,
) -> Json<HashMap<u32, Vec<JournalEntry>>> {
    let map = state
        .bot_id_to_journals
        .iter()
        .map(|r| (r.key().clone(), r.value().clone()))
        .collect::<HashMap<_, _>>();

    Json(map)
}

/// The handler for the HTTP request (this gets called when the HTTP request
/// lands at the start of websocket negotiation). After this completes, the
/// actual switching from HTTP to websocket protocol will occur.
/// This is the last point where we can extract TCP/IP metadata such as IP
/// address of the client as well as things from HTTP headers such as user-agent
/// of the browser etc.
async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");

    // Clone the receiver to pass to the handler
    let journal_rx = state.journal_rx.clone();

    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr, journal_rx))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(
    mut socket: WebSocket,
    who: SocketAddr,
    journal_rx: Receiver<JournalEntry>,
) {
    // send a ping (unsupported by some browsers) just to kick things off and
    // get a response
    // if socket
    //     .send(Message::Ping(Bytes::from_static(&[1, 2, 3])))
    //     .await
    //     .is_ok()
    // {
    //     println!("Pinged {who}...");
    // } else {
    //     println!("Could not send ping {who}!");
    //     // no Error here since the only thing we can do is to close the
    //     // connection. If we can not send messages, there is no way to
    //     // salvage the statemachine anyway.
    //     return;
    // }

    debug!("New websocket connection established with {}", who);

    // By splitting socket we can send and receive at the same time.
    let (mut sender, mut receiver) = socket.split();

    // Spawn a task that will handle incoming messages from the client
    let mut recv_task = tokio::spawn(async move {
        let mut cnt = 0;
        while let Some(Ok(msg)) = receiver.next().await {
            cnt += 1;
            trace!("Received message #{} from {}", cnt, who);
            // print message and break if instructed to do so
            if process_message(msg, who).is_break() {
                info!("Client {} requested to close the connection", who);
                break;
            }
        }
        debug!("Receiver task for {} completed after {} messages", who, cnt);
        cnt
    });

    // Spawn a task that will forward journal entries to the client
    let mut journal_task = tokio::spawn(async move {
        let mut cnt = 0;
        while let Ok(entry) = journal_rx.recv().await {
            cnt += 1;
            trace!("Forwarding journal entry #{} to {}", cnt, who);
            // Convert the journal entry to JSON and send it to the client
            match to_string(&entry) {
                Ok(json) => {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        warn!(
                            "Failed to send journal entry to {}, client \
                             disconnected",
                            who
                        );
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize journal entry: {}", e);
                }
            }
        }
        debug!("Journal task for {} completed after {} entries", who, cnt);
        cnt
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        rv_a = (&mut journal_task) => {
            match rv_a {
                Ok(a) => info!("{} journal entries sent to {}", a, who),
                Err(a) => error!("Error sending journal entries: {:?}", a)
            }
            debug!("Journal task completed, aborting receive task for {}", who);
            recv_task.abort();
        },
        rv_b = (&mut recv_task) => {
            match rv_b {
                Ok(b) => info!("Received {} messages from {}", b, who),
                Err(b) => error!("Error receiving messages: {:?}", b)
            }
            debug!("Receive task completed, aborting journal task for {}", who);
            journal_task.abort();
        }
    }

    // returning from the handler closes the websocket connection
    info!("Websocket connection with {} closed", who);
}

/// helper to print contents of messages to stdout. Has special treatment for
/// Close.
fn process_message(msg: Message, who: SocketAddr) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            println!(">>> {who} sent str: {t:?}");
        }
        Message::Binary(d) => {
            println!(">>> {} sent {} bytes: {:?}", who, d.len(), d);
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> {} sent close with code {} and reason `{}`",
                    who, cf.code, cf.reason
                );
            } else {
                println!(
                    ">>> {who} somehow sent close message without CloseFrame"
                );
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            println!(">>> {who} sent pong with {v:?}");
        }
        // You should never need to manually handle Message::Ping, as axum's
        // websocket library will do so for you automagically by
        // replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them
        // here.
        Message::Ping(v) => {
            println!(">>> {who} sent ping with {v:?}");
        }
    }
    ControlFlow::Continue(())
}
