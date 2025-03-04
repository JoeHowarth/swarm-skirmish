use bevy_utils::HashMap;
use bevy_ecs::component::Component;
pub use bevy_math;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

pub mod bot_harness;
pub mod protocol;
pub mod types;

pub use types::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SubscriptionType {
    Position,
    Radar,
    Team,
}

// Server -> Bot: Full update on each tick for subscribed data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerUpdate {
    // Always present
    pub tick: u32,

    // Subscribed state (only what the bot has requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub team: Option<Team>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub position: Option<Pos>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub radar: Option<RadarData>,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default)]
    pub items: HashMap<Item, u32>,

    // Results from previous actions
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub action_result: Option<ActionResult>,
    // Server messages (errors, notifications, etc.)
    // pub messages: Vec<ServerMessage>,
}

// Bot -> Server: Optional response with bundled fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotResponse {
    // Actions to take (empty vec if none)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub actions: Vec<ActionEnvelope>,

    // Subscription changes (both additions and removals)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub subscribe: Vec<SubscriptionType>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub unsubscribe: Vec<SubscriptionType>,
}

impl BotResponse {
    /// Create a new empty BotResponse
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            subscribe: Vec::new(),
            unsubscribe: Vec::new(),
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
)]
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