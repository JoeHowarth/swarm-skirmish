use serde::{Deserialize, Serialize};

use crate::{
    gridworld::{GridWorld, PassableCell},
    CellKind,
    FrameKind,
    Item,
    Pos,
    Subsystems,
    Team,
};

pub type KnownMap = GridWorld<ClientCellState>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientBotData {
    pub bot_id: u32,
    pub team: Team,
    pub last_observed: u32,
    /// World coordinates
    pub pos: Pos,
    pub frame: FrameKind,
    pub subsystems: Subsystems,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ClientCellState {
    pub kind: CellKind,
    // Optional bot_id
    pub pawn: Option<u32>,
    pub item: Option<Item>,
    pub last_observed: u32,
}

impl PassableCell for ClientCellState {
    fn is_blocked(&self) -> bool {
        self.pawn.is_some() || self.kind == CellKind::Blocked
    }
}

impl ClientCellState {
    pub fn is_unknown(&self) -> bool {
        self.kind == CellKind::Unknown
    }
}
