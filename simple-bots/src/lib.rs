#![allow(unused_imports, dead_code)]

use rand::{rngs::SmallRng, SeedableRng};
use swarm_lib::{bot_logger::BotLogger, Bot, NewBotNoMangeFn};

mod econ_bot;
mod old;

#[no_mangle]
pub fn test_fn() -> String {
    "Hello, world!".to_string()
}

#[no_mangle]
pub fn new_bot(ctx: BotLogger) -> Box<dyn Bot> {
    Box::new(econ_bot::EconBot {
        role: econ_bot::EconBotRole::default(),
        rng: SmallRng::seed_from_u64(ctx.bot_id as u64),
        ctx,
        action_counter: 0,
    })
}

/// Type check
static _X: NewBotNoMangeFn = new_bot;
