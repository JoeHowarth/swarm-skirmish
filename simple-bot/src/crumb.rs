use eyre::Result;
use rand::{
    rngs::{SmallRng, ThreadRng},
    Rng,
    SeedableRng,
};
use serde::{Deserialize, Serialize};
use swarm_lib::{
    bevy_math::UVec2,
    bot_harness::{Bot, Ctx},
    gridworld::{GridWorld, PassableCell},
    Action,
    ActionStatus,
    BotResponse,
    CellKind,
    CellStateRadar,
    Dir,
    Item::{self, *},
    Pos,
    ServerUpdate,
    Team,
};

use crate::{
    BotUpdate,
    ClientBotData,
    ClientCellState,
    CtxExt,
    MAP_HEIGHT,
    MAP_WIDTH,
};

/// Crumb follower seeks to move onto cells with Item::Fent
/// If no Fent is seen nearby, it follows the path of cells with Item::Crumb
/// until it finds Fent
pub struct CrumbFollower {
    // don't directly access this in BotUpdate
    ctx: Ctx,
    rng: SmallRng,
    default_dir: Dir,
    action_counter: u32,
    grid: GridWorld<ClientCellState>,
    seen_bots: Vec<ClientBotData>,
}

impl BotUpdate for CrumbFollower {
    fn update(&mut self, update: ServerUpdate) -> Option<BotResponse> {
        let radar = &update.radar;

        if let Some(result) = update.action_result {
            self.ctx.debug(format!("{result:?}"));

            if result.status == ActionStatus::InProgress {
                self.ctx
                    .info("Previous action still in progress, waiting...");
                return None;
            }
        }

        let action = if let Some((dir, _)) =
            radar.find_dirs(CellStateRadar::has_item(Fent))
        {
            self.ctx.debug("Found Fent");
            Action::MoveDir(dir)
        } else {
            if let Some((_, cell)) = radar.find(CellStateRadar::has_item(Crumb))
            {
                self.ctx.debug(format!(
                    "Found Crumb at world position: {}",
                    cell.pos
                ));

                Action::MoveTo(cell.pos)
            } else {
                // Generally moves in a consistent direction, but small chance
                // to change directions or change if going to hit wall
                if self.rng.random_bool(0.2)
                    || radar
                        .get_dir(self.default_dir)
                        .map(|cell| cell.kind)
                        .unwrap_or(CellKind::Blocked)
                        == CellKind::Blocked
                {
                    self.ctx.debug("Changing default dir");
                    self.default_dir =
                        Dir::from_repr(self.rng.random_range(0..=3)).unwrap();
                }
                self.ctx
                    .debug("No adjacent Fent or Crumb, moving to default dir");
                Action::MoveDir(self.default_dir)
            }
        };

        // Build and send response with movement action
        self.action_counter += 1;
        Some(
            BotResponse::builder()
                .push_action_id(action, self.action_counter)
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

impl Bot for CrumbFollower {
    fn new(ctx: Ctx) -> Self {
        Self {
            rng: SmallRng::seed_from_u64(ctx.bot_id as u64),
            ctx,
            default_dir: Dir::Up,
            action_counter: 0,
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
