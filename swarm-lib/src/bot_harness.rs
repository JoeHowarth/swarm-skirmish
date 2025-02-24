use std::{
    collections::HashMap,
    io::{BufReader, BufWriter, Write},
    net::TcpStream,
    process::exit,
    sync::mpsc::{self, Receiver, Sender},
};

use eyre::Result;

use crate::{
    protocol::Protocol,
    BotMsg,
    BotMsgEnvelope,
    ClientMsg,
    ResponseEnvelope,
    ServerMsg,
};

pub trait Bot {
    fn new(
        bot_id: u32,
        resp_rx: Receiver<ResponseEnvelope>,
        bot_msg_tx: Sender<BotMsgEnvelope>,
    ) -> Self;
    fn run(self) -> Result<()>;
}

pub fn run_bots<B: Bot + Send + 'static>() -> Result<()> {
    let writer = TcpStream::connect("127.0.0.1:1234")?;
    let mut reader = BufReader::new(writer.try_clone()?);
    let mut writer = BufWriter::new(writer);

    let (bot_msg_tx, bot_msg_rx) = mpsc::channel();

    std::thread::spawn(move || {
        let protocol = Protocol::new();
        let mut response_channel_map =
            HashMap::<u32, Sender<ResponseEnvelope>>::new();

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
                        let bot = B::new(bot_id, resp_rx, bot_msg_tx);
                        if let Err(e) = bot.run() {
                            eprintln!("Bot {} error: {:?}", bot_id, e);
                        }
                    });
                }
                ServerMsg::Response(response_envelope) => {
                    // Find the correct response channel for this bot
                    let resp_tx = response_channel_map
                        .get(&response_envelope.bot_id)
                        .unwrap();

                    // Forward the response to the bot
                    resp_tx
                        .send(response_envelope)
                        .expect("Failed to send response on channel");
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
