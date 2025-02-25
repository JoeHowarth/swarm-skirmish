use std::{
    collections::HashMap,
    io::{BufReader, BufWriter},
    net::TcpStream,
    process::exit,
    sync::mpsc::{self, Receiver, Sender}, thread::sleep, time::Duration,
};

use eyre::Result;

use crate::{
    protocol::Protocol,
    BotMsgEnvelope,
    BotResponse,
    CellStateRadar,
    ClientMsg,
    RadarData,
    ServerMsg,
    ServerUpdateEnvelope,
    Team,
};

/// Prints a visual representation of radar data to the terminal
pub fn print_radar(radar: &RadarData) {
    let width = radar.cells.num_columns();
    let height = radar.cells.num_rows();

    // Print top border
    println!("┌{}┐", "─".repeat(width * 2));

    // Print radar grid - FIXING coordinates: iterate y first, then x
    for y in 0..height {
        print!("│");
        for x in 0..width {
            // Get cell at (x,y) - not (y,x)
            let cell = radar.cells.get(x, height - y - 1).unwrap();
            match cell {
                CellStateRadar::Unknown => print!(". "),
                CellStateRadar::Empty => print!("  "),
                CellStateRadar::Blocked => print!("[]"),
                CellStateRadar::Bot { idx } => {
                    let bot = &radar.bots[*idx];
                    match bot.team {
                        Team::Player => print!("P "),
                        Team::Enemy => print!("E "),
                    }
                }
            }
        }
        println!("│");
    }

    // Print bottom border
    println!("└{}┘", "─".repeat(width * 2));

    // Print bot information
    if !radar.bots.is_empty() {
        println!("\nBots detected:");
        for (i, bot) in radar.bots.iter().enumerate() {
            println!(
                "  {}: {:?} at position ({}, {})",
                i, bot.team, bot.pos.x, bot.pos.y
            );
        }
    }
}

pub struct Rpc {
    pub bot_id: u32,
    pub seq: u32,
    pub resp_rx: Receiver<ServerUpdateEnvelope>,
    pub bot_msg_tx: Sender<BotMsgEnvelope>,
}

impl Rpc {
    pub fn new(
        bot_id: u32,
        resp_rx: Receiver<ServerUpdateEnvelope>,
        bot_msg_tx: Sender<BotMsgEnvelope>,
    ) -> Self {
        Rpc {
            bot_id,
            seq: 0,
            resp_rx,
            bot_msg_tx,
        }
    }

    pub fn wait_for_latest_update(&mut self) -> ServerUpdateEnvelope {
        let mut update = self
            .resp_rx
            .recv()
            .expect("Failed to receive server update");

        // drain the channel
        while let Ok(new_update) = self.resp_rx.try_recv() {
            update = new_update;
        }
        update
    }

    pub fn wait_for_update(&mut self) -> ServerUpdateEnvelope {
        self.resp_rx
            .recv()
            .expect("Failed to receive server update")
    }

    pub fn send_msg(&mut self, bot_msg: BotResponse) {
        let envelope = BotMsgEnvelope {
            bot_id: self.bot_id,
            seq: self.seq,
            msg: bot_msg,
        };

        // Increment sequence number for next message
        self.seq += 1;

        self.bot_msg_tx
            .send(envelope)
            .expect("Failed to send bot message");
    }

    /// Prints the radar data to the terminal if available in the server update
    pub fn print_radar(&self, update: &ServerUpdateEnvelope) {
        if let Some(radar) = &update.response.radar {
            println!(
                "Radar for Bot {} (Tick {}):",
                self.bot_id, update.response.tick
            );
            print_radar(radar);
        } else {
            println!(
                "No radar data available. Make sure to subscribe to radar \
                 updates."
            );
        }
    }
}

pub trait Bot {
    fn new(rpc: Rpc) -> Self;
    fn run(self) -> Result<()>;
}

pub fn run_bots<B: Bot + Send + 'static>() -> Result<()> {
    let writer;
    loop {
        if let Ok(writer_ok) = TcpStream::connect("127.0.0.1:1234") {
            writer = writer_ok;
            break;
        }
        sleep(Duration::from_millis(100));
    }

    let mut reader = BufReader::new(writer.try_clone()?);
    let mut writer = BufWriter::new(writer);

    let (bot_msg_tx, bot_msg_rx) = mpsc::channel();

    std::thread::spawn(move || {
        let protocol = Protocol::new();
        let mut response_channel_map =
            HashMap::<u32, Sender<ServerUpdateEnvelope>>::new();

        loop {
            let msg: ServerMsg = protocol.read_message(&mut reader).unwrap();
            match msg {
                ServerMsg::ConnectAck => println!("ConnectAck"),
                ServerMsg::AssignBot(bot_id) => {
                    println!("Got AssignBot msg: {bot_id}");

                    let (resp_tx, resp_rx) = mpsc::channel();
                    let bot_msg_tx = bot_msg_tx.clone();

                    response_channel_map.insert(bot_id, resp_tx);

                    std::thread::spawn(move || {
                        let rpc = Rpc::new(bot_id, resp_rx, bot_msg_tx);
                        let bot = B::new(rpc);
                        if let Err(e) = bot.run() {
                            eprintln!("Bot {} error: {:?}", bot_id, e);
                        }
                    });
                }
                ServerMsg::ServerUpdate(server_update_envelope) => {
                    // Find the correct response channel for this bot
                    let resp_tx = response_channel_map
                        .get(&server_update_envelope.bot_id)
                        .unwrap();

                    // Forward the response to the bot
                    resp_tx
                        .send(server_update_envelope)
                        .expect("Failed to send server update on channel");
                }
                ServerMsg::Close => {
                    println!("Received close message, exiting...");
                    exit(0);
                }
            }
        }
    });

    let protocol = Protocol::new();
    protocol
        .write_message(&mut writer, &ClientMsg::Connect)
        .expect("Failed to send Connect message");
    println!("Sent Connect msg to server");

    // Send bot messages to the server
    loop {
        let bot_msg: BotMsgEnvelope =
            bot_msg_rx.recv().expect("Failed to receive bot message");
        println!(
            "Received bot message on channel, sending to server...: msg: \
             {bot_msg:?}"
        );

        protocol
            .write_message(&mut writer, &ClientMsg::BotMsg(bot_msg))
            .expect("Failed to send bot message");

        println!("Bot msg sent to server");
    }
}
