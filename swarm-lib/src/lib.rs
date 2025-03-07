use bevy_ecs::component::Component;
pub use bevy_math;
use bevy_utils::HashMap;
use bot_logger::BotLogger;

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
    fn update(&mut self, update: BotUpdate) -> Option<ActionWithId>;
}

#[derive(Debug, Clone, Component)]
pub struct BotUpdate {
    pub tick: u32,

    pub team: Team,
    pub position: Pos,
    pub radar: RadarData,
    pub items: HashMap<Item, u32>,
    pub energy: Energy,

    // Result from previous action
    pub in_progress_action: Option<ActionWithId>,
    pub completed_action: Option<ActionResult>,
}

pub fn is_true(b: &bool) -> bool {
    *b
}
