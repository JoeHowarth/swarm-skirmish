use argh::FromArgs;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use swarm_lib::{Energy, Item, Pos, Team};

use super::Levels;
use crate::{
    types::{CellState, GridWorld, PawnKind},
    MAP_SIZE,
};

#[derive(
    FromArgs, Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq,
)]
#[argh(
    subcommand,
    name = "small-crumbs-and-truffles",
    description = "A small map with crumbs and truffles"
)]
pub struct SmallCrumbsAndTrufflesArgs {}

pub(super) fn init_small_crumbs_and_truffles(mut commands: Commands) {
    let (width, height) = (20, 20);
    *MAP_SIZE.write().unwrap() = Some((width, height));

    let mut grid_world = GridWorld::new(width, height, CellState::empty());

    let player = commands
        .spawn((
            PawnKind::default(),
            Team::Player,
            Energy(100),
            Pos((2, 2)),
        ))
        .id();

    // let enemy = commands
    //     .spawn((PawnKind::FindBot, Team::Enemy, Pos((13, 13).into())))
    //     .id();

    grid_world.set(2, 2, CellState::new_with_pawn(player));
    // grid_world.set(13, 13, CellState::new_with_pawn(enemy));

    for y in 1..10 {
        grid_world.set(10, y, CellState::blocked());
    }

    // Add crumbs
    for coord in grid_world.find_path((5, 3), (8, 13)).unwrap() {
        let cell = grid_world.get_pos_mut(coord);
        cell.item = Some(Item::Crumb);
    }
    // Add fent at end of crumb trail
    grid_world.get_mut(8, 14).item = Some(Item::Fent);

    grid_world.get_mut(2, 8).item = Some(Item::Truffle);
    grid_world.get_mut(12, 2).item = Some(Item::Truffle);

    // Add a border of Blocked cells around the edge of the grid
    for x in 0..width {
        // Top and bottom borders
        grid_world.set(x, 0, CellState::blocked());
        grid_world.set(x, height - 1, CellState::blocked());
    }

    for y in 0..height {
        // Left and right borders
        grid_world.set(0, y, CellState::blocked());
        grid_world.set(width - 1, y, CellState::blocked());
    }

    commands.insert_resource(grid_world);
}
