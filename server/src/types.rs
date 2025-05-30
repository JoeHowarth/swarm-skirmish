use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use swarm_lib::{
    gridworld::{self, PassableCell},
    known_map::ClientCellState,
    CellKind,
    FrameKind,
    Item,
    Pos,
    Subsystems,
    Team,
};

use crate::game::bot_update::BotIdToEntity;

pub type GridWorld = gridworld::GridWorld<CellState>;

#[derive(Resource, Default)]
pub struct Tick(pub u32);

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct PartiallyBuiltBot {
    pub frame_kind: FrameKind,
    pub subsystems: Subsystems,
    pub pos: Pos,
    pub team: Team,
    pub _ticks_required: u32,
    pub ticks_remaining: u32,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub struct CellState {
    pub kind: CellKind,
    pub partially_built_bot: Option<Entity>,
    pub pawn: Option<Entity>,
    pub item: Option<Item>,
}

impl CellState {
    pub fn empty() -> CellState {
        CellState {
            kind: CellKind::Empty,
            ..default()
        }
    }

    pub fn blocked() -> CellState {
        CellState {
            kind: CellKind::Blocked,
            ..default()
        }
    }

    pub fn new_with_pawn(pawn: Entity) -> CellState {
        CellState {
            kind: CellKind::Empty,
            pawn: Some(pawn),
            ..default()
        }
    }

    pub fn can_enter(&self) -> bool {
        self.kind == CellKind::Empty
            && self.pawn.is_none()
            && self.partially_built_bot.is_none()
    }

    pub fn from_client_state(
        state: &ClientCellState,
        bot_id_map: &BotIdToEntity,
    ) -> CellState {
        CellState {
            kind: state.kind,
            partially_built_bot: None, // TODO: fixme
            pawn: state.pawn.map(bot_id_map.u32()),
            item: state.item,
        }
    }
}

impl PassableCell for CellState {
    fn is_blocked(&self) -> bool {
        !self.can_enter()
    }
}
