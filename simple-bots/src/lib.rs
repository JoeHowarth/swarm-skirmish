#![allow(unused_imports, dead_code)]

use rand::{rngs::SmallRng, SeedableRng};
use swarm_lib::{
    bot_logger::BotLogger,
    gridworld::GridWorld,
    known_map::ClientCellState,
    Bot,
    Dir,
    Item::*,
    NewBotNoMangeFn,
};

mod econ_bot;
mod old;

#[no_mangle]
pub fn test_fn() -> String {
    "Hello, world!".to_string()
}

#[no_mangle]
pub fn new_bot(ctx: BotLogger, (map_w, map_h): (usize, usize)) -> Box<dyn Bot> {
    Box::new(econ_bot::EconBot {
        role: econ_bot::EconBotRole::default(),
        grid: GridWorld::new(map_w, map_h, ClientCellState::default()),
        rng: SmallRng::seed_from_u64(ctx.bot_id as u64),
        ctx,
        action_counter: 0,
        seen_bots: Vec::new(),
    })
}

/// Type check
static _X: NewBotNoMangeFn = new_bot;
