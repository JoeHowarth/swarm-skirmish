use array2d::Array2D;
use bevy_ecs::component::Component;
pub use bevy_math;
use bevy_math::{UVec2};
use serde::{Deserialize, Serialize};

pub mod bot_harness;
pub mod protocol;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarBotData {
    pub team: Team,
    pub pos: UVec2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CellStateRadar {
    Unknown,
    Empty,
    Blocked,
    Bot { idx: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarData {
    pub bots: Vec<RadarBotData>,
    pub cells: Array2D<CellStateRadar>,
}

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
    pub team: Option<Team>,
    pub position: Option<UVec2>,
    pub radar: Option<RadarData>,
    // pub team_status: Option<TeamStatus>,
    // pub resources: Option<ResourceData>,

    // Results from previous actions
    // pub action_results: Vec<ActionResult>,

    // Server messages (errors, notifications, etc.)
    // pub messages: Vec<ServerMessage>,
}

// Bot -> Server: Optional response with bundled fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotResponse {
    // Actions to take (empty vec if none)
    pub actions: Vec<Action>,

    // Subscription changes (both additions and removals)
    pub subscribe: Vec<SubscriptionType>,
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

/// Builder for BotResponse to enable fluent method chaining
#[derive(Debug, Clone, Default)]
pub struct BotResponseBuilder {
    actions: Vec<Action>,
    subscribe: Vec<SubscriptionType>,
    unsubscribe: Vec<SubscriptionType>,
}

impl BotResponseBuilder {
    /// Create a new empty BotResponseBuilder
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            subscribe: Vec::new(),
            unsubscribe: Vec::new(),
        }
    }

    /// Add an action to the response
    pub fn push_action(&mut self, action: Action) -> &mut Self {
        self.actions.push(action);
        self
    }

    /// Add a subscription to the response
    pub fn subscribe(&mut self, subscription: SubscriptionType) -> &mut Self {
        self.subscribe.push(subscription);
        self
    }

    /// Add an unsubscription to the response
    pub fn unsubscribe(&mut self, subscription: SubscriptionType) -> &mut Self {
        self.unsubscribe.push(subscription);
        self
    }

    /// Build the final BotResponse
    pub fn build(&mut self) -> BotResponse {
        BotResponse {
            actions: std::mem::take(&mut self.actions),
            subscribe: std::mem::take(&mut self.subscribe),
            unsubscribe: std::mem::take(&mut self.unsubscribe),
        }
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
    pub seq: u32,
    pub msg: BotResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    pub fn to_deltas(&self) -> (isize, isize) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }

    pub fn from_deltas(deltas: (isize, isize)) -> Option<Self> {
        match (deltas.0, deltas.1) {
            (0, -1) => Some(Dir::Up),
            (0, 1) => Some(Dir::Down),
            (-1, 0) => Some(Dir::Left),
            (1, 0) => Some(Dir::Right),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    MoveDir(Dir),
    MoveTo(UVec2), // WaitUntilTick(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    ConnectAck,
    AssignBot(u32),
    ServerUpdate(ServerUpdateEnvelope),
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerUpdateEnvelope {
    pub bot_id: u32,
    pub seq: u32,
    pub response: ServerUpdate,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Team {
    Player,
    Enemy,
}
