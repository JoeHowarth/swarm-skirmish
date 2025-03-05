use std::fmt::Write;

use bevy::{prelude::*, utils::HashMap};
use strum_macros::Display;
use swarm_lib::{
    gridworld::{GridWorld, PassableCell},
    CellKind,
    Item,
    Pos,
};

use crate::server::BotId;
pub type SGridWorld = GridWorld<CellState>;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct CoreSystemsSet;

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tick>().add_systems(
            Update,
            (update_tick, pickup_fent, pickup_crumbs).in_set(CoreSystemsSet),
        );
    }
}

#[derive(Resource, Default)]
pub struct Tick(pub u32);

#[derive(Component, Default, Display, Copy, Clone)]
#[require(Inventory)]
pub enum PawnKind {
    #[default]
    Basic,
    FindBot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellState {
    pub kind: CellKind,
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

fn update_tick(mut tick: ResMut<Tick>) {
    tick.0 += 1;
    debug!("Tick: {}", tick.0);
}

#[derive(Component, Default, Deref, DerefMut)]
pub struct Inventory(pub HashMap<Item, u32>);

fn pickup_crumbs(
    mut grid_world: ResMut<SGridWorld>,
    mut pawns: Query<(&PawnKind, &BotId, &mut Inventory, &Pos)>,
) {
    for (pawn_kind, bot_id, mut inventory, pos) in pawns.iter_mut() {
        let cell = grid_world.get_pos_mut(*pos);
        if let Some(Item::Crumb) = cell.item {
            info!(%pos, ?bot_id, %pawn_kind, "Picking up Crumb");
            *inventory.0.entry(Item::Crumb).or_default() += 1;
            cell.item = None;
        };
    }
}

fn pickup_fent(
    mut grid_world: ResMut<SGridWorld>,
    mut pawns: Query<(&PawnKind, &BotId, &mut Inventory, &Pos)>,
) {
    for (pawn_kind, bot_id, mut inventory, pos) in pawns.iter_mut() {
        let cell = grid_world.get_pos_mut(*pos);
        if let Some(Item::Fent) = cell.item {
            info!(%pos, ?bot_id, %pawn_kind, "Picking up Fent");
            *inventory.0.entry(Item::Fent).or_default() += 1;
            cell.item = None;
        };
    }
}
