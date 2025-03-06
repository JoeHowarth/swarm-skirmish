use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    net::{TcpListener, TcpStream},
    sync::{mpmc, Arc, Mutex},
};

use bevy::{prelude::*, utils::HashMap};
use eyre::{bail, Context, Result};
use serde::Serialize;
use swarm_lib::{
    protocol::Protocol,
    Action,
    ActionEnvelope,
    BotMsgEnvelope,
    ClientMsg,
    JournalEntry,
    ServerMsg,
    ServerUpdateEnvelope,
    Team,
};

use crate::{
    actions::{ActionQueue, ComputedActionQueue, InProgressAction},
    core::PawnKind,
    get_map_size,
    subscriptions::Subscriptions,
    MAP_SIZE,
};

#[derive(Component, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[require(Subscriptions, ActionQueue, InProgressAction, ComputedActionQueue)]
pub struct BotId(pub u32);

#[derive(Resource)]
pub struct NewBots(pub mpmc::Sender<(u32, PawnKind)>);

#[derive(Resource)]
pub struct KillBots(pub mpmc::Sender<u32>);

#[derive(Resource)]
pub struct ServerUpdates(pub mpmc::Sender<ServerUpdateEnvelope>);

#[derive(Resource)]
pub struct ActionRecv(pub mpmc::Receiver<(BotId, u32, ActionEnvelope)>);

#[derive(Resource, Default)]
pub struct BotIdToEntity(pub HashMap<BotId, Entity>);

pub struct BotHandlerPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct ServerSystems;

impl Plugin for BotHandlerPlugin {
    fn build(&self, app: &mut App) {
        let (new_bots_tx, new_bots_rx) = mpmc::channel();
        let (server_update_tx, server_update_rx) = mpmc::channel();
        let (action_tx, action_rx) = mpmc::channel();
        let (kill_bots_tx, kill_bots_rx) = mpmc::channel();

        app.world_mut()
            .register_component_hooks::<BotId>()
            .on_remove(|mut world, entity, _| {
                let bot_id = *world.get::<BotId>(entity).unwrap();

                // Remove bot_id from bot_id_to_entity
                let mut bot_id_to_entity =
                    world.get_resource_mut::<BotIdToEntity>().unwrap();
                bot_id_to_entity.0.remove(&bot_id);

                // Send kill message to bot
                // world.resource::<KillBots>().0.send(bot_id.0).unwrap();
            });

        app.init_resource::<BotIdToEntity>();
            // .insert_resource(NewBots(new_bots_tx))
            // .insert_resource(ServerUpdates(server_update_tx))
            // .insert_resource(ActionRecv(action_rx))
            // .insert_resource(KillBots(kill_bots_tx));

        app.add_systems(Update, add_new_bots.in_set(ServerSystems));

        std::thread::spawn(move || {
            server(new_bots_rx, server_update_rx, action_tx, kill_bots_rx)
        });
    }
}

fn add_new_bots(
    mut commands: Commands,
    without_id: Query<(Entity, &Team, &PawnKind), Without<BotId>>,
    new_bots: Res<NewBots>,
    mut bot_id_to_entity: ResMut<BotIdToEntity>,
    mut next_id: Local<u32>,
) {
    for (entity, _team, kind) in without_id.iter() {
        *next_id += 1;
        let id = BotId(*next_id);

        // send to server, to send to client to assign bot
        new_bots.0.send((id.0, *kind)).unwrap();

        // Add bot_id
        bot_id_to_entity.0.insert(id, entity);
        commands.entity(entity).insert(id);
        info!("New botId sent to channel");
    }
}

fn server(
    new_bots_rx: mpmc::Receiver<(u32, PawnKind)>,
    server_update_rx: mpmc::Receiver<ServerUpdateEnvelope>,
    action_tx: mpmc::Sender<(BotId, u32, ActionEnvelope)>,
    kill_bots_rx: mpmc::Receiver<u32>,
) {
    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        let new_bots_rx = new_bots_rx.clone();
        let server_update_rx = server_update_rx.clone();
        let action_tx = action_tx.clone();
        let kill_bots_rx = kill_bots_rx.clone();

        std::thread::spawn(move || {
            if let Err(e) = handle_connection(
                stream,
                new_bots_rx,
                kill_bots_rx,
                server_update_rx,
                action_tx,
            ) {
                eprintln!("Connection error: {:?}", e);
            }
        });
    }
}

fn handle_connection(
    stream: TcpStream,
    new_bots_rx: mpmc::Receiver<(u32, PawnKind)>,
    kill_bots_rx: mpmc::Receiver<u32>,
    server_update_rx: mpmc::Receiver<ServerUpdateEnvelope>,
    action_tx: mpmc::Sender<(BotId, u32, ActionEnvelope)>,
) -> Result<()> {
    let (mut reader, mut writer) =
        create_protocol_handlers(stream, "./journal.json").unwrap();

    let connect = reader.read_message()?;
    if !matches!(connect, ClientMsg::Connect) {
        bail!("Expected Connect message, got: {connect:?}");
    }

    // Send ConnectAck
    loop {
        if let Some(map_size) = get_map_size() {
            writer.write_message(&ServerMsg::ConnectAck { map_size })?;
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    std::thread::spawn(move || {
        let mut reader = reader;
        loop {
            let msg: ClientMsg = reader.read_message().unwrap();
            match msg {
                ClientMsg::Connect => error!("Sent 'Connect' msg twice"),
                ClientMsg::BotMsg(BotMsgEnvelope { bot_id, tick, msg }) => {
                    // Process bot response
                    // Actions
                    for action in msg.actions {
                        action_tx.send((BotId(bot_id), tick, action)).unwrap();
                    }
                }
            }
        }
    });

    loop {
        if let Ok((new_bot_id, pawn_kind)) = new_bots_rx.try_recv() {
            info!("Assigning bot id {new_bot_id}, kind  {pawn_kind}");
            writer
                .write_message(&ServerMsg::AssignBot(
                    new_bot_id,
                    pawn_kind.to_string(),
                ))
                .unwrap();
        }

        if let Ok(server_update) = server_update_rx.try_recv() {
            trace!(
                "Sending server update to bot: {}, seq: {}",
                server_update.bot_id,
                server_update.seq
            );
            writer
                .write_message(&ServerMsg::ServerUpdate(server_update))
                .unwrap();
        }

        if let Ok(bot_id) = kill_bots_rx.try_recv() {
            info!("Killing bot: {}", bot_id);
            writer.write_message(&ServerMsg::KillBot(bot_id)).unwrap();
        }
    }
}

pub fn create_protocol_handlers(
    stream: TcpStream,
    journal_path: &str,
) -> Result<(ProtocolReader, ProtocolWriter)> {
    let journal = MessageJournal::new(journal_path)?;
    let journal = Some(Arc::new(Mutex::new(journal)));

    let reader_stream = stream.try_clone()?;
    let writer_stream = stream;

    let reader = ProtocolReader::new(reader_stream, journal.clone());
    let writer = ProtocolWriter::new(writer_stream, journal);

    Ok((reader, writer))
}

pub struct ProtocolReader {
    reader: BufReader<TcpStream>,
    journal: Option<Arc<Mutex<MessageJournal>>>,
}

impl ProtocolReader {
    pub fn new(
        stream: TcpStream,
        journal: Option<Arc<Mutex<MessageJournal>>>,
    ) -> Self {
        Self {
            reader: BufReader::new(stream),
            journal,
        }
    }

    pub fn read_message(&mut self) -> Result<ClientMsg> {
        let msg = Protocol::read_message(&mut self.reader)?;

        // Journal the message if a journal is provided
        if let Some(journal) = &self.journal {
            let bot_id = match &msg {
                ClientMsg::Connect => None,
                ClientMsg::BotMsg(envelope) => Some(envelope.bot_id),
            };
            journal.lock().unwrap().log_client_message(bot_id, &msg);
        }

        Ok(msg)
    }
}

pub struct ProtocolWriter {
    writer: BufWriter<TcpStream>,
    journal: Option<Arc<Mutex<MessageJournal>>>,
}

impl ProtocolWriter {
    pub fn new(
        stream: TcpStream,
        journal: Option<Arc<Mutex<MessageJournal>>>,
    ) -> Self {
        Self {
            writer: BufWriter::new(stream),
            journal,
        }
    }

    pub fn write_message(&mut self, msg: &ServerMsg) -> Result<()> {
        Protocol::write_message(&mut self.writer, msg)?;
        self.writer.flush()?;

        // Journal the message if a journal is provided
        if let Some(journal) = &self.journal {
            let bot_id = match msg {
                ServerMsg::AssignBot(id, _) => Some(*id),
                ServerMsg::ServerUpdate(update) => Some(update.bot_id),
                _ => None,
            };
            journal.lock().unwrap().log_server_message(bot_id, msg);
        }

        Ok(())
    }
}

#[derive(Resource)]
pub struct MessageJournal {
    file: Arc<Mutex<File>>,
}

impl MessageJournal {
    pub fn new(path: &str) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .wrap_err("Failed to open journal file")?;

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
        })
    }

    pub fn log_client_message(&self, bot_id: Option<u32>, msg: &ClientMsg) {
        let entry = JournalEntry {
            timestamp: chrono::Local::now().to_rfc3339(),
            bot_id,
            client_msg: Some(msg.clone()),
            server_msg: None,
        };

        self.write_entry(&entry);
    }

    pub fn log_server_message(&self, bot_id: Option<u32>, msg: &ServerMsg) {
        let entry = JournalEntry {
            timestamp: chrono::Local::now().to_rfc3339(),
            bot_id,
            client_msg: None,
            server_msg: Some(msg.clone()),
        };

        self.write_entry(&entry);
    }

    fn write_entry(&self, entry: &JournalEntry) {
        let Ok(file) = self.file.lock() else {
            return;
        };
        let mut writer = BufWriter::new(&*file);
        serde_json::to_writer(&mut writer, entry).unwrap();
        writer.write(b"\n").unwrap();
        // bincode::serialize_into(&mut writer, entry).unwrap();
        writer.flush().unwrap();
    }
}
