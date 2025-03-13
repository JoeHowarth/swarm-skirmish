use argh::{FromArgValue, FromArgs};
use bevy::{prelude::*, state::state::FreelyMutableState};
use econ_loop::EconLoopArgs;
use rand::{prelude::SliceRandom, Rng};
use random_crumbs_and_truffles::{
    init_random_crumbs_and_truffles,
    RandomCrumbsAndTrufflesArgs,
};
use serde::{Deserialize, Serialize};
use small_crumbs_and_truffles::{
    init_small_crumbs_and_truffles,
    SmallCrumbsAndTrufflesArgs,
};
use strum_macros::EnumDiscriminants;
use swarm_lib::{Energy, Item, Pos, Team};

use crate::{
    types::{CellState, GridWorld},
    GameState,
    MAP_SIZE,
};

mod econ_loop;
mod random_crumbs_and_truffles;
mod small_crumbs_and_truffles;

pub struct LevelsPlugin;

impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(LevelsDiscriminants::SmallCrumbsAndTruffles),
            (init_small_crumbs_and_truffles, transition_to_in_game),
        )
        .add_systems(
            OnEnter(LevelsDiscriminants::RandomCrumbsAndTruffles),
            (init_random_crumbs_and_truffles, transition_to_in_game),
        )
        .add_systems(
            OnEnter(LevelsDiscriminants::EconLoop),
            (econ_loop::init_econ_loop, transition_to_in_game),
        );
    }
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Hash,
    Eq,
    PartialEq,
    strum_macros::EnumDiscriminants,
    FromArgs,
    Resource,
)]
#[argh(subcommand)]
#[strum_discriminants(derive(Hash))]
pub enum Levels {
    SmallCrumbsAndTruffles(SmallCrumbsAndTrufflesArgs),
    RandomCrumbsAndTruffles(RandomCrumbsAndTrufflesArgs),
    EconLoop(EconLoopArgs),
}

impl Default for Levels {
    fn default() -> Self {
        Self::SmallCrumbsAndTruffles(SmallCrumbsAndTrufflesArgs {})
    }
}

impl States for LevelsDiscriminants {
    const DEPENDENCY_DEPTH: usize = 1;
}

impl FreelyMutableState for LevelsDiscriminants {}

impl FromArgValue for LevelsDiscriminants {
    fn from_arg_value(value: &str) -> Result<Self, String> {
        Ok(match value {
            "small-crumbs-and-truffles" => {
                LevelsDiscriminants::SmallCrumbsAndTruffles
            }
            "random-crumbs-and-truffles" => {
                LevelsDiscriminants::RandomCrumbsAndTruffles
            }
            "econ-loop" => LevelsDiscriminants::EconLoop,
            _ => return Err(format!("Invalid level: {}", value)),
        })
    }
}

fn transition_to_in_game(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::InGame);
}
