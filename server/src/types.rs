use bevy::{prelude::*, utils::HashMap};
use strum_macros::Display;
use swarm_lib::{
    gridworld::{self, PassableCell},
    BuildingKind,
    CellKind,
    Energy,
    FrameKind,
    Item,
    Pos,
    Subsystem,
    Subsystems,
    Team,
};

pub type GridWorld = gridworld::GridWorld<CellState>;

#[derive(Resource, Default)]
pub struct Tick(pub u32);

#[derive(Component, Clone)]
pub struct PartiallyBuiltBot {
    pub frame_kind: FrameKind,
    pub subsystems: Subsystems,
    pub pos: Pos,
    pub team: Team,
    pub _ticks_required: u32,
    pub ticks_remaining: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

    #[allow(dead_code)]
    pub fn new_with_item(item: Item) -> CellState {
        CellState {
            kind: CellKind::Empty,
            item: Some(item),
            ..default()
        }
    }

    pub fn can_enter(&self) -> bool {
        self.kind == CellKind::Empty && self.pawn.is_none()
    }
}

impl PassableCell for CellState {
    fn is_blocked(&self) -> bool {
        !self.can_enter()
    }
}
