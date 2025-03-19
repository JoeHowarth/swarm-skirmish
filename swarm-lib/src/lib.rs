#![allow(unused_imports)]
#![feature(try_trait_v2)]
// #![feature(generic_const_exprs)]

use std::ops::{ControlFlow, FromResidual, Try};

use bevy_ecs::component::Component;
pub use bevy_math;
use bot_logger::BotLogger;

pub mod bot_logger;
pub mod gridworld;
pub mod known_map;
pub mod radar;
pub mod types;

use known_map::{ClientBotData, KnownMap};
pub use radar::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumCount, EnumDiscriminants, FromRepr};
pub use types::*;
use ustr::Ustr;

pub type NewBotNoMangeFn = fn(logger: BotLogger) -> Box<dyn Bot>;

pub trait Bot: Sync + Send + 'static {
    fn update(&mut self, update: BotUpdate) -> Option<ActionWithId>;
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct BotData {
    pub frame: FrameKind,
    pub subsystems: Subsystems,
    pub energy: Energy,
    pub inventory: Inventory,
    pub pos: Pos,
    pub team: Team,
    pub known_map: KnownMap,
    pub known_bots: Vec<ClientBotData>,
}

#[derive(Debug, Clone, Component)]
pub struct BotUpdate {
    pub tick: u32,

    pub bot_data: BotData,

    // Result from previous action
    pub in_progress_action: Option<ActionWithId>,
    pub completed_action: Option<ActionResult>,
}

pub type ActionId = u32;

#[derive(Debug, Clone)]
pub struct ActionWithId {
    pub id: ActionId,
    pub action: Action,
    pub reason: &'static str,
}

#[derive(Debug, Clone, EnumDiscriminants, Serialize, Deserialize)]
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

impl BotData {
    pub fn new(
        frame_kind: FrameKind,
        subsystems: Subsystems,
        pos: Pos,
        team: Team,
        energy: Energy,
        known_map: KnownMap,
        known_bots: Vec<ClientBotData>,
    ) -> Self {
        assert!(
            subsystems.size() <= frame_kind.slots(),
            "Subsystems require more slots than the frame kind has. Slots: \
             {}, Subsystems: {}",
            frame_kind.slots(),
            subsystems.size()
        );
        Self {
            frame: frame_kind,
            energy,
            inventory: Inventory::new(subsystems.get(Subsystem::CargoBay), []),
            subsystems,
            pos,
            team,
            known_map,
            known_bots,
        }
    }

    pub fn max_energy(&self) -> Energy {
        let base = match self.frame {
            FrameKind::Flea => 100,
            FrameKind::Tractor => 100,
            FrameKind::Building(BuildingKind::Small) => 500,
        };

        let power_cell_count = self.subsystems.get(Subsystem::PowerCell) as u32;

        Energy(base + power_cell_count * 100)
    }

    pub fn is_capable_of(&self, action: &Action) -> bool {
        match action {
            Action::Noop => true,
            Action::MoveDir(_) | Action::MoveTo(_) => match self.frame {
                FrameKind::Flea => true,
                FrameKind::Tractor => true,
                FrameKind::Building(_building_kind) => false,
            },
            Action::Harvest(_dir) => {
                self.subsystems.has(Subsystem::MiningDrill)
            }
            Action::Pickup(_) => self.subsystems.has(Subsystem::CargoBay),
            Action::Drop(_) => self.subsystems.has(Subsystem::CargoBay),
            Action::Transfer(_) => self.subsystems.has(Subsystem::CargoBay),
            Action::Build(_dir, _frame_kind, _subsystems) => {
                self.subsystems.has(Subsystem::Assembler)
            }
            Action::Recharge(_dir) => true,
            Action::Attack(_dir) => self.subsystems.has(Subsystem::PlasmaRifle),
        }
    }
}

#[derive(Default, Display, Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameKind {
    #[default]
    Flea,
    Tractor,
    Building(BuildingKind),
}

impl FrameKind {
    pub const fn build_cost(&self) -> u8 {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumCount, FromRepr)]
#[repr(u8)]
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

    pub fn build_cost(&self) -> u8 {
        self.slots_required()
    }
}

impl From<Subsystem> for u8 {
    fn from(value: Subsystem) -> Self {
        value as u8
    }
}

impl From<u8> for Subsystem {
    fn from(value: u8) -> Self {
        Subsystem::from_repr(value).unwrap()
    }
}

impl std::fmt::Debug for Subsystems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Subsystems {{")?;
        for (subsystem, count) in self.iter() {
            if count > 0 {
                write!(f, "{:?}: {},", subsystem, count)?;
            }
        }
        write!(f, "}}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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
                let subsystems_cost: u8 = subsystems
                    .iter()
                    .map(|(s, num)| s.build_cost() * num)
                    .sum();
                Some((frame_kind_cost + subsystems_cost) as u32)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: Action,
    pub id: ActionId,
    pub status: ActionStatus,
    pub reason: Ustr,
    pub completed_tick: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, strum_macros::EnumDiscriminants, Serialize, Deserialize)]
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
    Act(Action, &'static str),
}

impl DecisionResult {
    pub fn or_continue(
        self,
        next: impl FnOnce() -> DecisionResult,
    ) -> DecisionResult {
        match self {
            DecisionResult::Continue => next(),
            _ => self,
        }
    }
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
            decision @ (DecisionResult::Wait | DecisionResult::Act(_, _)) => {
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
