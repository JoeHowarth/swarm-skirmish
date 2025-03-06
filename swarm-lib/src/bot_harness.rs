use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    net::TcpStream,
    path::PathBuf,
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
        Mutex,
    },
    thread::sleep,
    time::Duration,
};

use bevy_utils::tracing::trace;
use chrono::Local;
use eyre::Result;
use once_cell::sync::Lazy;

use crate::{
    ctx::Ctx,
    protocol::Protocol,
    BotMsgEnvelope,
    BotResponse,
    CellKind,
    ClientMsg,
    RadarData,
    ServerMsg,
    ServerUpdate,
    ServerUpdateEnvelope,
    Team,
};

/// Global map size, initialized when ConnectAck is received
static MAP_SIZE: Lazy<Mutex<Option<(usize, usize)>>> =
    Lazy::new(|| Mutex::new(None));

pub fn map_size() -> (usize, usize) {
    MAP_SIZE
        .lock()
        .expect("cannot acquire MAP_SIZE lock")
        .expect("Tried to get MAP_SIZE before ConnectAck")
}

/// Log level for bot logs

pub trait Bot: Sync + Send + 'static + std::fmt::Debug {
    fn update(&mut self, update: ServerUpdate) -> Option<BotResponse>;
}

pub trait OldBot: Send + 'static {
    fn new(ctx: Ctx) -> Self
    where
        Self: Sized;
    fn run(&mut self) -> Result<()>;
}

type BotFactory = Box<dyn Fn(Ctx) -> Box<dyn OldBot> + Send>;

pub struct Harness {
    factories: HashMap<String, BotFactory>,
}

impl Harness {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    pub fn register<B: OldBot>(&mut self, name: impl Into<String>) -> &mut Self {
        self.factories
            .insert(name.into(), Box::new(move |ctx| Box::new(B::new(ctx))));

        self
    }

    pub fn handle_connection(&mut self, stream: TcpStream) -> Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut writer = BufWriter::new(stream);

        let (bot_msg_tx, bot_msg_rx) = mpsc::channel();
        let factories = &mut self.factories;
        let mut kill_signals = HashMap::new();

        std::thread::scope(|s| {
            let shutdown_signal = Arc::new(AtomicBool::new(false));
            let shutdown_signal_reader = shutdown_signal.clone();

            s.spawn(move || {
                let mut response_channel_map =
                    HashMap::<u32, Sender<ServerUpdateEnvelope>>::new();

                loop {
                    if shutdown_signal_reader.load(Ordering::SeqCst) {
                        return;
                    }

                    let Ok(msg) = Protocol::read_message(&mut reader) else {
                        eprintln!("Failed to read message. Shutting down...");
                        shutdown_signal_reader.store(true, Ordering::SeqCst);
                        return;
                    };
                    match msg {
                        ServerMsg::ConnectAck { map_size } => {
                            println!(
                                "ConnectAck with map_size: {:?}",
                                map_size
                            );
                            // Set the global MAP_SIZE
                            if let Ok(mut global_map_size) = MAP_SIZE.lock() {
                                *global_map_size = Some(map_size);
                            } else {
                                eprintln!("Failed to set global MAP_SIZE");
                            }
                        }
                        ServerMsg::AssignBot(bot_id, bot_type) => {
                            println!(
                                "Got AssignBot msg: {bot_id} (type: \
                                 {bot_type}), spawning bot..."
                            );

                            // Set up channels and Ctx
                            let (resp_tx, resp_rx) = mpsc::channel();
                            let bot_msg_tx = bot_msg_tx.clone();
                            let kill_signal = Arc::new(AtomicBool::new(false));
                            kill_signals.insert(bot_id, kill_signal.clone());
                            let ctx = Ctx::new(
                                bot_id,
                                resp_rx,
                                bot_msg_tx,
                                kill_signal,
                            );

                            response_channel_map.insert(bot_id, resp_tx);

                            // Use factory to create bot
                            let factory = factories.get(&bot_type).unwrap();
                            let mut bot = factory(ctx);

                            // Spawn bot
                            std::thread::spawn(move || {
                                std::thread::sleep(Duration::from_millis(20));
                                if let Err(e) = bot.run() {
                                    eprintln!(
                                        "[Error] Bot {} error: {:?}",
                                        bot_id, e
                                    );
                                }
                            });
                            println!("Bot Spawned: {bot_id}");
                        }
                        ServerMsg::ServerUpdate(server_update_envelope) => {
                            // Find the correct response channel for this bot
                            let resp_tx = response_channel_map
                                .get(&server_update_envelope.bot_id)
                                .unwrap();

                            // Forward the response to the bot
                            resp_tx.send(server_update_envelope).expect(
                                "Failed to send server update on channel",
                            );
                        }
                        ServerMsg::Close => {
                            println!("Received close message, exiting...");
                            shutdown_signal_reader.store(true, Ordering::SeqCst);
                            return;
                        }
                        ServerMsg::KillBot(bot_id) => {
                            if let Some(kill_signal) =
                                kill_signals.remove(&bot_id)
                            {
                                println!(
                                    "Received kill message for bot {bot_id}, \
                                     killing bot..."
                                );
                                kill_signal.store(true, Ordering::SeqCst);
                            } else {
                                println!(
                                    "Received kill message for bot {bot_id}, \
                                     but bot not found"
                                );
                            }
                        }
                    }
                }
            });

            s.spawn(move || {
                if let Err(e) =
                    Protocol::write_message(&mut writer, &ClientMsg::Connect)
                {
                    eprintln!("Failed to send Connect message: {e}");
                    shutdown_signal.store(true, Ordering::SeqCst);
                    return;
                }

                println!("Sent Connect msg to server");

                // Send bot messages to the server
                loop {
                    if shutdown_signal.load(Ordering::SeqCst) {
                        return;
                    }

                    let Ok(bot_msg) =
                        bot_msg_rx.recv_timeout(Duration::from_secs(1))
                    else {
                        continue;
                    };

                    println!(
                        "Received bot message on channel, sending to \
                         server...: msg: {bot_msg:?}"
                    );

                    if let Err(e) = Protocol::write_message(
                        &mut writer,
                        &ClientMsg::BotMsg(bot_msg),
                    ) {
                        eprintln!("Failed to send bot message: {e}");
                        shutdown_signal.store(true, Ordering::SeqCst);
                        return;
                    }

                    println!("Bot msg sent to server");
                }
            });
        });

        Ok(())
    }

    pub fn run_bots(mut self) -> Result<()> {
        loop {
            if let Ok(writer_ok) = TcpStream::connect("127.0.0.1:1234") {
                println!("Connected to server");
                if let Err(e) = self.handle_connection(writer_ok) {
                    eprintln!("Failed to handle connection: {e}");
                }
            }
            sleep(Duration::from_millis(100));
        }
    }
}
