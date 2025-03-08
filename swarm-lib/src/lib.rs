#![feature(try_trait_v2)]

use std::ops::{ControlFlow, FromResidual, Try};

use bevy_ecs::component::Component;
pub use bevy_math;
use bevy_utils::HashMap;
use bot_logger::BotLogger;

pub mod bot_logger;
pub mod gridworld;
pub mod known_map;
pub mod radar;
pub mod types;

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

pub type ActionId = u32;

#[derive(Debug, Clone)]
pub struct ActionWithId {
    pub id: ActionId,
    pub action: Action,
}

#[derive(Debug, Clone)]
pub enum Action {
    Noop,
    MoveDir(Dir),
    MoveTo(Vec<Pos>),
    Harvest(Dir),
}

impl Action {
    pub fn total_energy(&self) -> Energy {
        match self {
            Action::MoveDir(_) => 1,
            Action::MoveTo(path) => path.len() as u32,
            Action::Harvest(_) => 5,
            Action::Noop => 0,
        }
        .into()
    }

    pub fn energy_per_tick(&self) -> Energy {
        match self {
            Action::MoveDir(_) => 1,
            Action::MoveTo(_) => 1,
            Action::Harvest(_) => 5,
            Action::Noop => 0,
        }
        .into()
    }
}

#[derive(Debug, Clone)]
pub struct ActionResult {
    pub action: Action,
    pub id: ActionId,
    pub status: ActionStatus,
    pub completed_tick: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, strum_macros::EnumDiscriminants)]
pub enum ActionStatus {
    Success,
    Failure(String),
    Cancelled,
}

impl ActionStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, ActionStatus::Success)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, ActionStatus::Failure(_))
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, ActionStatus::Cancelled)
    }
}

pub enum DecisionResult {
    /// No decision made, continue to next behavior
    Continue,
    /// Wait for current action to complete
    Wait,
    /// Perform a new action
    Act(Action),
}

impl Try for DecisionResult {
    type Output = ();
    type Residual = Self; // Use DecisionResult as its own residual

    fn from_output(_: Self::Output) -> Self {
        DecisionResult::Continue
    }

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            DecisionResult::Continue => ControlFlow::Continue(()),
            decision @ (DecisionResult::Wait | DecisionResult::Act(_)) => {
                ControlFlow::Break(decision)
            }
        }
    }
}

impl FromResidual for DecisionResult {
    fn from_residual(residual: Self) -> Self {
        residual // Just return the residual directly
    }
}
