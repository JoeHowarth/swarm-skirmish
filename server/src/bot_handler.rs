use std::{
    io::{BufReader, BufWriter},
    net::{TcpListener, TcpStream},
    sync::{
        mpmc,
        mpsc::{Receiver, Sender},
    },
};

use bevy::prelude::*;
use eyre::{bail, Result};
use swarm_lib::{
    bot_harness::Bot,
    protocol::{Connection, Protocol},
    Action,
    BotMsg,
    BotMsgEnvelope,
    ClientMsg,
    QueryEnvelope,
    ResponseEnvelope,
    ServerMsg,
    Team,
};

pub struct BotHandlerPlugin;

impl Plugin for BotHandlerPlugin {
    fn build(&self, app: &mut App) {
        let (new_bots_tx, new_bots_rx) = mpmc::channel();
        app.insert_resource(NewBots(new_bots_tx));

        let (bot_resp_tx, bot_resp_rx) = mpmc::channel();
        app.insert_resource(BotResponses(bot_resp_tx));

        let (action_tx, action_rx) = mpmc::channel();
        app.insert_resource(ActionRecv(action_rx));

        let (query_tx, query_rx) = mpmc::channel();
        app.insert_resource(QueryRecv(query_rx));

        app.add_systems(Update, add_new_bots);

        std::thread::spawn(move || {
            server(new_bots_rx, bot_resp_rx, action_tx, query_tx)
        });
    }
}

#[derive(Component, Debug)]
pub struct BotId(pub u32);

fn add_new_bots(
    mut commands: Commands,
    without_id: Query<(Entity, &Team), Without<BotId>>,
    new_bots: Res<NewBots>,
    mut next_id: Local<u32>,
) {
    for (entity, _team) in without_id.iter() {
        *next_id += 1;
        let id = BotId(*next_id);
        new_bots.0.send(id.0).unwrap();
        commands.entity(entity).insert(id);
        info!("New botId sent to channel");
    }
}

#[derive(Resource)]
pub struct NewBots(pub mpmc::Sender<u32>);

#[derive(Resource)]
pub struct BotResponses(pub mpmc::Sender<ResponseEnvelope>);

#[derive(Resource)]
pub struct ActionRecv(pub mpmc::Receiver<(BotId, Action)>);

#[derive(Resource)]
pub struct QueryRecv(pub mpmc::Receiver<(BotId, QueryEnvelope)>);

fn server(
    new_bots_rx: mpmc::Receiver<u32>,
    bot_resp_rx: mpmc::Receiver<ResponseEnvelope>,
    action_tx: mpmc::Sender<(BotId, Action)>,
    query_tx: mpmc::Sender<(BotId, QueryEnvelope)>,
) {
    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        let new_bots_rx = new_bots_rx.clone();
        let bot_resp_rx = bot_resp_rx.clone();
        let action_tx = action_tx.clone();
        let query_tx = query_tx.clone();

        std::thread::spawn(move || {
            if let Err(e) = handle_connection(
                stream,
                new_bots_rx,
                bot_resp_rx,
                action_tx,
                query_tx,
            ) {
                eprintln!("Connection error: {:?}", e);
            }
        });
    }
}

fn handle_connection(
    stream: TcpStream,
    new_bots_rx: mpmc::Receiver<u32>,
    bot_resp_rx: mpmc::Receiver<ResponseEnvelope>,
    action_tx: mpmc::Sender<(BotId, Action)>,
    query_tx: mpmc::Sender<(BotId, QueryEnvelope)>,
) -> Result<()> {
    let protocol = Protocol::new();

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = BufWriter::new(stream);

    let connect: ClientMsg = protocol.read_message(&mut reader)?;
    if !matches!(connect, ClientMsg::Connect) {
        bail!("Expected Connect message, got: {connect:?}");
    }

    std::thread::spawn(move || {
        let mut reader = reader;
        loop {
            let msg: ClientMsg = protocol.read_message(&mut reader).unwrap();
            match msg {
                ClientMsg::Connect => error!("Sent 'Connect' msg twice"),
                ClientMsg::BotMsg(BotMsgEnvelope { bot_id, msg }) => {
                    match msg {
                        BotMsg::Action(action) => {
                            action_tx.send((BotId(bot_id), action)).unwrap()
                        }
                        BotMsg::Query(query_envelope) => query_tx
                            .send((BotId(bot_id), query_envelope))
                            .unwrap(),
                    }
                }
            }
        }
    });

    loop {
        if let Ok(new_bot_id) = new_bots_rx.try_recv() {
            info!("Assigning bot id {new_bot_id}");
            protocol
                .write_message(&mut writer, &ServerMsg::AssignBot(new_bot_id))
                .unwrap();
        }

        if let Ok(bot_resp) = bot_resp_rx.try_recv() {
            info!(
                "Sending bot response to bot: {}, query_seq: {}",
                bot_resp.bot_id, bot_resp.query_seq
            );
            protocol
                .write_message(&mut writer, &ServerMsg::Response(bot_resp))
                .unwrap();
        }
    }
}
