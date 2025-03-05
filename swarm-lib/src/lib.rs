use bevy_ecs::component::Component;
pub use bevy_math;
use bevy_utils::HashMap;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

pub mod bot_harness;
pub mod gridworld;
pub mod protocol;
pub mod types;

pub use types::*;

// Server -> Bot: Full update on each tick with all data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerUpdate {
    // Always present
    pub tick: u32,

    // Bot state - always included now
    pub team: Team,
    pub position: Pos,
    pub radar: RadarData,
    pub items: HashMap<Item, u32>,

    // Results from previous actions
    #[serde(default)]
    pub action_result: Option<ActionResult>,
    // Server messages (errors, notifications, etc.)
    // pub messages: Vec<ServerMessage>,
}

// Bot -> Server: Optional response with actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotResponse {
    // Actions to take (empty vec if none)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub actions: Vec<ActionEnvelope>,
}

impl BotResponse {
    /// Create a new empty BotResponse
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Create a new BotResponseBuilder
    pub fn builder() -> BotResponseBuilder {
        BotResponseBuilder::new()
    }
}

///////////////

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMsg {
    Connect,
    BotMsg(BotMsgEnvelope),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotMsgEnvelope {
    pub bot_id: u32,
    pub tick: u32,
    pub msg: BotResponse,
}

#[derive(
    Debug,
    Copy,
    Clone,
    Serialize,
    Deserialize,
    strum_macros::EnumIter,
    strum_macros::FromRepr,
    strum_macros::EnumDiscriminants,
)]
#[repr(u8)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

pub type ActionId = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEnvelope {
    pub id: ActionId,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    MoveDir(Dir),
    MoveTo(Pos),
    Harvest(Dir),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: Action,
    pub id: ActionId,
    pub status: ActionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActionStatus {
    Success,
    Failure,
    InProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    ConnectAck,
    AssignBot(u32, String),
    ServerUpdate(ServerUpdateEnvelope),
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerUpdateEnvelope {
    pub bot_id: u32,
    pub seq: u32,
    pub response: ServerUpdate,
}

#[derive(
    Component,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Display,
)]
pub enum Team {
    Player,
    Enemy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub timestamp: String,
    pub bot_id: Option<u32>,
    pub client_msg: Option<ClientMsg>,
    pub server_msg: Option<ServerMsg>,
}
