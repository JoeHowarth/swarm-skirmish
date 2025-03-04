use eyre::Result;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use swarm_lib::{
    bot_harness::{Bot, Ctx},
    gridworld::GridWorld,
    Action,
    BotResponse,
    CellStateRadar,
    Dir,
    RadarBotData,
    ServerUpdate,
};

use crate::{
    BotUpdate,
    ClientBotData,
    ClientCellState,
    CtxExt,
    MAP_HEIGHT,
    MAP_WIDTH,
};

pub struct RandomWalkBot {
    ctx: Ctx,
    rng: SmallRng,
    grid: GridWorld<ClientCellState>,
    seen_bots: Vec<ClientBotData>,
}

impl Bot for RandomWalkBot {
    fn new(ctx: Ctx) -> Self {
        Self {
            rng: SmallRng::seed_from_u64(ctx.bot_id as u64),
            ctx: ctx,
            grid: GridWorld::new(
                MAP_WIDTH,
                MAP_HEIGHT,
                ClientCellState::default(),
            ),
            seen_bots: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<()> {
        crate::run_loop(self)
    }
}

impl BotUpdate for RandomWalkBot {
    fn update(&mut self, _update: ServerUpdate) -> Option<BotResponse> {
        // Choose a random direction to move
        let direction = match self.rng.random_range(0..4) {
            0 => Dir::Up,
            1 => Dir::Down,
            2 => Dir::Left,
            3 => Dir::Right,
            _ => unreachable!(),
        };

        self.ctx.info(format!("Moving {:?}", direction));

        // Build and send response with random movement
        Some(
            BotResponse::builder()
                .push_action(Action::MoveDir(direction))
                .build(),
        )
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }

    fn known_map(
        &mut self,
    ) -> (&mut GridWorld<ClientCellState>, &mut Vec<ClientBotData>) {
        (&mut self.grid, &mut self.seen_bots)
    }
}
