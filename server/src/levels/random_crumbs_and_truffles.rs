use argh::FromArgs;
use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use swarm_lib::{
    known_map::{ClientCellState, KnownMap},
    BotData,
    Energy,
    FrameKind,
    Item,
    Pos,
    Subsystem,
    Subsystems,
    Team,
};

use super::Levels;
use crate::{
    graphics::tilemap::MapSize,
    types::{CellState, GridWorld},
};

#[derive(
    FromArgs, Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq,
)]
#[argh(
    subcommand,
    name = "random-crumbs-and-truffles",
    description = "A random map with crumbs and truffles"
)]
pub struct RandomCrumbsAndTrufflesArgs {
    #[argh(option, default = "20")]
    /// the width of the map
    pub width: usize,
    #[argh(option, default = "20")]
    /// the height of the map
    pub height: usize,
}

pub(super) fn init_random_crumbs_and_truffles(
    mut commands: Commands,
    level_args: Res<Levels>,
) {
    let args = match &*level_args {
        Levels::RandomCrumbsAndTruffles(args) => args,
        _ => panic!("Expected RandomCrumbsAndTruffles level"),
    };

    let (width, height) = (args.width, args.height);
    commands.insert_resource(MapSize {
        x: width as u32,
        y: height as u32,
    });

    let mut grid_world = GridWorld::new(width, height, CellState::empty());
    let mut rng = rand::rng();

    // Add a border of Blocked cells around the edge of the grid
    for x in 0..width {
        // Top and bottom borders
        grid_world.set_tuple(x, 0, CellState::blocked());
        grid_world.set_tuple(x, height - 1, CellState::blocked());
    }

    for y in 0..height {
        // Left and right borders
        grid_world.set_tuple(0, y, CellState::blocked());
        grid_world.set_tuple(width - 1, y, CellState::blocked());
    }

    // Generate 5 sets of contiguous wall segments
    for _ in 0..5 {
        let segment_length = rng.random_range(2..=20);
        let start_x = rng.random_range(2..width - 2);
        let start_y = rng.random_range(2..height - 2);

        // Choose a random direction: 0=right, 1=down, 2=left, 3=up
        let direction = rng.random_range(0..4);

        for i in 0..segment_length {
            let (wall_x, wall_y) = match direction {
                0 => (start_x + i, start_y),               // right
                1 => (start_x, start_y + i),               // down
                2 => (start_x.saturating_sub(i), start_y), // left
                3 => (start_x, start_y.saturating_sub(i)), // up
                _ => unreachable!(),
            };

            // Check bounds to avoid panic
            if wall_x >= 1
                && wall_x < width - 1
                && wall_y >= 1
                && wall_y < height - 1
            {
                grid_world.set_tuple(wall_x, wall_y, CellState::blocked());
            }
        }
    }

    // Helper function to find empty cells
    let mut find_empty_cell = |grid: &GridWorld| -> (usize, usize) {
        loop {
            let x = rng.random_range(1..width - 1);
            let y = rng.random_range(1..height - 1);

            let cell = grid.get_tuple(x, y);
            if cell.can_enter() && cell.pawn.is_none() && cell.item.is_none() {
                return (x, y);
            }
        }
    };

    // Place 2 bots of the same team
    let team = Team::Player;

    // Place first bot
    let (bot1_x, bot1_y) = find_empty_cell(&grid_world);
    let bot1 = commands
        .spawn(BotData::new(
            FrameKind::default(),
            Subsystems::new([(Subsystem::CargoBay, 1)]),
            Pos((bot1_x, bot1_y)),
            team,
            Energy(100),
            KnownMap::new(width, height, ClientCellState::default()),
            Vec::new(),
        ))
        .id();
    grid_world.set_tuple(bot1_x, bot1_y, CellState::new_with_pawn(bot1));

    // Place second bot
    let (bot2_x, bot2_y) = find_empty_cell(&grid_world);
    let bot2 = commands
        .spawn(BotData::new(
            FrameKind::default(),
            Subsystems::new([(Subsystem::CargoBay, 1)]),
            Pos((bot2_x, bot2_y)),
            team,
            Energy(100),
            KnownMap::new(width, height, ClientCellState::default()),
            Vec::new(),
        ))
        .id();
    grid_world.set_tuple(bot2_x, bot2_y, CellState::new_with_pawn(bot2));

    // Place 2 Fent items
    for _ in 0..2 {
        let (x, y) = find_empty_cell(&grid_world);
        grid_world.get_tuple_mut(x, y).item = Some(Item::Fent);
    }

    // Place 3 Truffle items
    for _ in 0..3 {
        let (x, y) = find_empty_cell(&grid_world);
        grid_world.get_tuple_mut(x, y).item = Some(Item::Truffle);
    }

    commands.insert_resource(grid_world);
}
