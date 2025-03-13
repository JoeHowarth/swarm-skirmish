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
use strum_macros::Display;
pub use types::*;

pub type NewBotNoMangeFn =
    fn(logger: BotLogger, map_size: (usize, usize)) -> Box<dyn Bot>;

pub trait Bot: Sync + Send + 'static + std::fmt::Debug {
    fn update(&mut self, update: BotUpdate) -> Option<ActionWithId>;
}

#[derive(Debug, Clone, Component)]
pub struct BotData {
    pub frame_kind: FrameKind,
    pub subsystems: Subsystems,
    pub energy: Energy,
    pub inventory: Inventory,
    pub pos: Pos,
    pub team: Team,
}

#[derive(Debug, Clone, Component)]
pub struct BotUpdate {
    pub tick: u32,

    pub bot_data: BotData,
    pub radar: RadarData,

    // Result from previous action
    pub in_progress_action: Option<ActionWithId>,
    pub completed_action: Option<ActionResult>,
}

pub fn is_true(b: &bool) -> bool {
    *b
}

#[derive(Default, Debug, Clone)]
pub struct Inventory(pub HashMap<Item, u32>);

#[derive(Default, Debug, Clone)]
pub struct Subsystems(pub HashMap<Subsystem, u8>);

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
    Pickup((Item, Option<Dir>)),
    Drop((Item, Option<Dir>)),
    Transfer((Item, Dir)),
    Build(Dir, BuildingKind),
    Recharge,
    Attack(Dir),
}

#[derive(Default, Display, Copy, Clone, Debug)]
pub enum FrameKind {
    #[default]
    Basic,
    Building(BuildingKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subsystem {
    PlasmaRifle,
    PowerCell,
    CargoBay,
    PrecisionOptics,
    OpticalTransciever,
    MiningDrill,
    Assembler,
}

impl Subsystem {
    pub fn slots_required(&self) -> u32 {
        match self {
            Subsystem::PlasmaRifle => 1,
            Subsystem::PowerCell => 1,
            Subsystem::CargoBay => 1,
            Subsystem::PrecisionOptics => 2,
            Subsystem::OpticalTransciever => 2,
            Subsystem::MiningDrill => 3,
            Subsystem::Assembler => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildingKind {
    #[default]
    Base,
}

impl Action {
    pub fn ticks_to_complete(&self) -> Option<u32> {
        match self {
            Action::MoveDir(_) => Some(1),
            Action::MoveTo(path) => Some(path.len() as u32 - 1),
            Action::Harvest(_) => Some(1),
            Action::Noop => Some(1),
            Action::Pickup(_) => Some(1),
            Action::Drop(_) => Some(1),
            Action::Transfer(_) => Some(1),
            Action::Build(_, BuildingKind::Base) => None,
            Action::Recharge => None,
            Action::Attack(_) => Some(1),
        }
    }

    pub fn energy_per_tick(&self) -> Energy {
        match self {
            Action::MoveDir(_) => 1.into(),
            Action::MoveTo(_) => 1.into(),
            Action::Harvest(_) => 5.into(),
            Action::Noop => 0.into(),
            Action::Pickup(_) => 2.into(),
            Action::Drop(_) => 1.into(),
            Action::Transfer(_) => 2.into(),
            Action::Build(_, BuildingKind::Base) => 2.into(),
            Action::Recharge => 0.into(),
            Action::Attack(_) => 4.into(),
        }
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
