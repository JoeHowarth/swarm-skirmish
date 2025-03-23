use std::ops::{Deref, DerefMut};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownMap {
    pub map: GridWorld<ClientCellState>,
    pub last_received_map_from: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ClientCellState {
    pub kind: CellKind,
    // Optional bot_id
    pub pawn: Option<u32>,
    pub item: Option<Item>,
    pub last_observed: u32,
}

impl KnownMap {
    pub fn update_from(&mut self, other: &Self, from: u32) {
        for (pos, theirs) in other.iter() {
            let ours = self.get_mut(Pos::from(pos));
            if theirs.last_observed > ours.last_observed {
                *ours = theirs.clone();
            }
        }
        self.last_received_map_from = Some(from);
    }

    pub fn new(width: usize, height: usize, default: ClientCellState) -> Self {
        Self {
            map: GridWorld::new(width, height, default),
            last_received_map_from: None,
        }
    }
}

impl Deref for KnownMap {
    type Target = GridWorld<ClientCellState>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl DerefMut for KnownMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
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
