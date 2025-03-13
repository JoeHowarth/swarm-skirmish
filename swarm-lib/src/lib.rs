#![allow(unused_imports)]
#![feature(try_trait_v2)]

use std::ops::{ControlFlow, FromResidual, Try};

use bevy_ecs::component::Component;
pub use bevy_math;
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
    Build(Dir, FrameKind, Subsystems),
    Recharge(Dir),
    Attack(Dir),
}

#[derive(Default, Display, Copy, Clone, Debug)]
pub enum FrameKind {
    #[default]
    Flea,
    Tractor,
    Building(BuildingKind),
}

impl BotData {
    pub fn new(
        frame_kind: FrameKind,
        subsystems: Subsystems,
        pos: Pos,
        team: Team,
    ) -> Self {
        Self {
            frame_kind,
            subsystems,
            energy: 100.into(),
            inventory: Inventory::default(),
            pos,
            team,
        }
    }

    pub fn max_energy(&self) -> Energy {
        let base = match self.frame_kind {
            FrameKind::Flea => 100,
            FrameKind::Tractor => 100,
            FrameKind::Building(BuildingKind::Small) => 100,
        };

        let power_cell_count =
            *self.subsystems.get(&Subsystem::PowerCell).unwrap_or(&0) as u32;

        Energy(base + power_cell_count * 100)
    }

    pub fn is_capable_of(&self, action: &Action) -> bool {
        match action {
            Action::Noop => true,
            Action::MoveDir(_) | Action::MoveTo(_) => match self.frame_kind {
                FrameKind::Flea => true,
                FrameKind::Tractor => true,
                FrameKind::Building(_building_kind) => false,
            },
            Action::Harvest(_dir) => {
                self.subsystems.contains_key(&Subsystem::MiningDrill)
            }
            Action::Pickup(_) => {
                self.subsystems.contains_key(&Subsystem::CargoBay)
            }
            Action::Drop(_) => {
                self.subsystems.contains_key(&Subsystem::CargoBay)
            }
            Action::Transfer(_) => {
                self.subsystems.contains_key(&Subsystem::CargoBay)
            }
            Action::Build(_dir, _frame_kind, _subsystems) => {
                self.subsystems.contains_key(&Subsystem::Assembler)
            }
            Action::Recharge(_dir) => true,
            Action::Attack(_dir) => {
                self.subsystems.contains_key(&Subsystem::PlasmaRifle)
            }
        }
    }
}

impl FrameKind {
    pub const fn build_cost(&self) -> u32 {
        match self {
            FrameKind::Flea => 2,
            FrameKind::Tractor => 5,
            FrameKind::Building(BuildingKind::Small) => 7,
        }
    }

    pub const fn slots(&self) -> u8 {
        match self {
            FrameKind::Flea => 1,
            FrameKind::Tractor => 6,
            FrameKind::Building(BuildingKind::Small) => 10,
        }
    }

    pub fn is_building(&self) -> bool {
        matches!(self, FrameKind::Building(_))
    }

    pub fn is_basic(&self) -> bool {
        matches!(self, FrameKind::Flea)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subsystem {
    PlasmaRifle,
    PowerCell,
    CargoBay,
    PrecisionOptics,
    OpticalTransciever,
    MiningDrill,
    Assembler,
    Generator,
}

impl Subsystem {
    pub fn slots_required(&self) -> u8 {
        match self {
            Subsystem::PlasmaRifle => 1,
            Subsystem::PowerCell => 1,
            Subsystem::CargoBay => 1,
            Subsystem::PrecisionOptics => 2,
            Subsystem::OpticalTransciever => 2,
            Subsystem::MiningDrill => 3,
            Subsystem::Assembler => 5,
            Subsystem::Generator => 5,
        }
    }

    pub fn build_cost(&self) -> u32 {
        self.slots_required() as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildingKind {
    #[default]
    Small,
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
            Action::Build(_dir, frame_kind, subsystems) => {
                let frame_kind_cost = frame_kind.build_cost();
                let subsystems_cost: u32 = subsystems
                    .iter()
                    .map(|(s, num)| s.build_cost() * *num as u32)
                    .sum();
                Some(frame_kind_cost + subsystems_cost)
            }
            Action::Recharge(_dir) => None,
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
            Action::Build(_dir, _frame_kind, _subsystems) => 2.into(),
            Action::Recharge(_dir) => 0.into(),
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
