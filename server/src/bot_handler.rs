use std::{
    io::{BufReader, BufWriter},
    net::{TcpListener, TcpStream},
    sync::{
        mpmc,
        mpsc::{Receiver, Sender},
    },
};

use bevy::{prelude::*, utils::HashMap};
use eyre::{bail, Result};
use swarm_lib::{
    protocol::{Connection, Protocol},
    Action,
    BotMsgEnvelope,
    BotResponse,
    ClientMsg,
    ServerMsg,
    ServerUpdate,
    ServerUpdateEnvelope,
    Team,
};

use crate::BotId;

pub struct BotHandlerPlugin;

impl Plugin for BotHandlerPlugin {
    fn build(&self, app: &mut App) {
        let (new_bots_tx, new_bots_rx) = mpmc::channel();
        let (server_update_tx, server_update_rx) = mpmc::channel();
        let (action_tx, action_rx) = mpmc::channel();
        let (subscription_tx, subscription_rx) = mpmc::channel();

        app.init_resource::<BotIdToEntity>()
            .insert_resource(NewBots(new_bots_tx))
            .insert_resource(ServerUpdates(server_update_tx))
            .insert_resource(ActionRecv(action_rx))
            .insert_resource(SubscriptionRecv(subscription_rx));

        app.add_systems(Update, add_new_bots);

        std::thread::spawn(move || {
            server(new_bots_rx, server_update_rx, action_tx, subscription_tx)
        });
    }
}

#[derive(Resource, Default)]
pub struct BotIdToEntity(pub HashMap<BotId, Entity>);

fn add_new_bots(
    mut commands: Commands,
    without_id: Query<(Entity, &Team), Without<BotId>>,
    new_bots: Res<NewBots>,
    mut bot_id_to_entity: ResMut<BotIdToEntity>,
    mut next_id: Local<u32>,
) {
    for (entity, _team) in without_id.iter() {
        *next_id += 1;
        let id = BotId(*next_id);
        new_bots.0.send(id.0).unwrap();
        bot_id_to_entity.0.insert(id, entity);
        commands.entity(entity).insert(id);
        info!("New botId sent to channel");
    }
}

#[derive(Resource)]
pub struct NewBots(pub mpmc::Sender<u32>);

#[derive(Resource)]
pub struct ServerUpdates(pub mpmc::Sender<ServerUpdateEnvelope>);

#[derive(Resource)]
pub struct ActionRecv(pub mpmc::Receiver<(BotId, Action)>);

#[derive(Resource)]
pub struct SubscriptionRecv(
    pub mpmc::Receiver<(BotId, Vec<swarm_lib::SubscriptionType>)>,
);

fn server(
    new_bots_rx: mpmc::Receiver<u32>,
    server_update_rx: mpmc::Receiver<ServerUpdateEnvelope>,
    action_tx: mpmc::Sender<(BotId, Action)>,
    subscription_tx: mpmc::Sender<(BotId, Vec<swarm_lib::SubscriptionType>)>,
) {
    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        let new_bots_rx = new_bots_rx.clone();
        let server_update_rx = server_update_rx.clone();
        let action_tx = action_tx.clone();
        let subscription_tx = subscription_tx.clone();

        std::thread::spawn(move || {
            if let Err(e) = handle_connection(
                stream,
                new_bots_rx,
                server_update_rx,
                action_tx,
                subscription_tx,
            ) {
                eprintln!("Connection error: {:?}", e);
            }
        });
    }
}

fn handle_connection(
    stream: TcpStream,
    new_bots_rx: mpmc::Receiver<u32>,
    server_update_rx: mpmc::Receiver<ServerUpdateEnvelope>,
    action_tx: mpmc::Sender<(BotId, Action)>,
    subscription_tx: mpmc::Sender<(BotId, Vec<swarm_lib::SubscriptionType>)>,
) -> Result<()> {
    let protocol = Protocol::new();

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = BufWriter::new(stream);

    let connect: ClientMsg = protocol.read_message(&mut reader)?;
    if !matches!(connect, ClientMsg::Connect) {
        bail!("Expected Connect message, got: {connect:?}");
    }

    // Send ConnectAck
    protocol.write_message(&mut writer, &ServerMsg::ConnectAck)?;

    std::thread::spawn(move || {
        let mut reader = reader;
        loop {
            let msg: ClientMsg = protocol.read_message(&mut reader).unwrap();
            match msg {
                ClientMsg::Connect => error!("Sent 'Connect' msg twice"),
                ClientMsg::BotMsg(BotMsgEnvelope {
                    bot_id,
                    seq: _seq,
                    msg,
                }) => {
                    // Process bot response
                    for action in msg.actions {
                        action_tx.send((BotId(bot_id), action)).unwrap();
                    }

                    // Handle subscriptions
                    if !msg.subscribe.is_empty() {
                        subscription_tx
                            .send((BotId(bot_id), msg.subscribe))
                            .unwrap();
                    }

                    // Handle unsubscriptions if needed
                    if !msg.unsubscribe.is_empty() {
                        // Add handling for unsubscriptions if needed
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

        if let Ok(server_update) = server_update_rx.try_recv() {
            info!(
                "Sending server update to bot: {}, seq: {}",
                server_update.bot_id, server_update.seq
            );
            protocol
                .write_message(
                    &mut writer,
                    &ServerMsg::ServerUpdate(server_update),
                )
                .unwrap();
        }
    }
}
