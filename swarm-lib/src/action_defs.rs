
use serde::{Deserialize, Serialize};

use crate::{Dir, Energy, Pos};

pub type ActionId = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionWithId {
    pub id: ActionId,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: Action,
    pub id: ActionId,
    pub status: ActionStatus,
    pub completed_tick: u32,
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
