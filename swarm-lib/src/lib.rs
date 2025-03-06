use bevy_ecs::component::Component;
pub use bevy_math;
use bevy_utils::HashMap;
use bot_logger::BotLogger;
use serde::{Deserialize, Serialize};

pub mod action_defs;
pub mod bot_logger;
pub mod gridworld;
pub mod radar;
pub mod types;

pub use action_defs::*;
pub use radar::*;
pub use types::*;

pub type NewBotNoMangeFn =
    fn(logger: BotLogger, map_size: (usize, usize)) -> Box<dyn Bot>;

pub trait Bot: Sync + Send + 'static + std::fmt::Debug {
    fn update(&mut self, update: BotUpdate) -> Option<BotResp>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Component)]
pub struct BotUpdate {
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BotResp {
    // Actions to take
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub actions: Vec<ActionWithId>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub cancel_actions: Vec<ActionId>,

    #[serde(default, skip_serializing_if = "crate::is_true")]
    pub cancel_all_actions: bool,
}

impl BotResp {
    /// Create a new empty BotResponse
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new BotResponseBuilder
    pub fn builder() -> RespBuilder {
        RespBuilder::new()
    }
}

pub fn is_true(b: &bool) -> bool {
    *b
}
