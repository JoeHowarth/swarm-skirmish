use bevy_ecs::component::Component;
pub use bevy_math;
use bevy_utils::HashMap;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

pub mod boring_impls;
pub mod bot_harness;
pub mod ctx;
pub mod gridworld;
pub mod protocol;
pub mod types;

pub use types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    ConnectAck { map_size: (usize, usize) },
    AssignBot(u32, String),
    KillBot(u32),
    ServerUpdate(ServerUpdateEnvelope),
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerUpdateEnvelope {
    pub bot_id: u32,
    pub seq: u32,
    pub response: ServerUpdate,
}
// Server -> Bot: Full update on each tick with all data
#[derive(Debug, Clone, Serialize, Deserialize, Component)]
pub struct ServerUpdate {
    pub tick: u32,

    pub team: Team,
    pub position: Pos,
    pub radar: RadarData,
    pub items: HashMap<Item, u32>,
    pub energy: Energy,

    // Results from previous actions
    #[serde(default)]
    pub action_result: Option<ActionResult>,
}

// Bot -> Server: Optional response with actions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BotResponse {
    // Actions to take
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub actions: Vec<ActionEnvelope>,
    // #[serde(skip_serializing_if = "Vec::is_empty")]
    // #[serde(default)]
    // pub cancel_actions: Vec<ActionId>,

    // #[serde(default, skip_serializing_if = "is_true")]
    // pub cancel_all_actions: bool,
}

impl BotResponse {
    /// Create a new empty BotResponse
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new BotResponseBuilder
    pub fn builder() -> BotResponseBuilder {
        BotResponseBuilder::new()
    }
}

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

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    strum_macros::EnumDiscriminants,
)]
pub enum ActionStatus {
    Success,
    Failure(String),
    InProgress { progress: u16, total: u16 },
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

fn is_true(b: &bool) -> bool {
    *b
}
