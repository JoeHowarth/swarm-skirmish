use eyre::Result;
use rand::{
    rngs::{SmallRng, ThreadRng},
    Rng,
    SeedableRng,
};
use serde::{Deserialize, Serialize};
use swarm_lib::{
    bevy_math::UVec2,
    bot_harness::{map_size, Bot, Ctx},
    gridworld::{GridWorld, PassableCell},
    Action,
    ActionStatus,
    ActionStatusDiscriminants,
    BotResponse,
    CellKind,
    CellStateRadar,
    Dir,
    Item::{self, *},
    Pos,
    RadarData,
    ServerUpdate,
    Team,
};

use crate::{BotUpdate, ClientBotData, ClientCellState, CtxExt};

pub struct CrumbFollower {
    ctx: Ctx,
    rng: SmallRng,
    default_dir: Dir,
    action_counter: u32,
    grid: GridWorld<ClientCellState>,
    seen_bots: Vec<ClientBotData>,
}

impl CrumbFollower {
    // Helper method to determine the next action with a linear flow instead of
    // nested conditionals
    fn determine_next_action(
        &mut self,
        radar: &RadarData,
        tick: u32,
    ) -> Action {
        // Priority 1: Find and move toward Fent
        if let Some((dir, _)) = radar.find_dirs(CellStateRadar::has_item(Fent))
        {
            self.ctx.debug("Found Fent");
            return Action::MoveDir(dir);
        }

        // Priority 2: Harvest nearby Truffle
        if let Some((dir, _)) =
            radar.find_dirs(CellStateRadar::has_item(Truffle))
        {
            self.ctx.debug(format!("Harvesting Truffle {dir:?}"));
            return Action::Harvest(dir);
        }

        // Priority 3: Follow Crumb
        if let Some((_, cell)) = radar.find(CellStateRadar::has_item(Crumb)) {
            self.ctx
                .debug(format!("Found Crumb at world position: {}", cell.pos));
            return Action::MoveTo(cell.pos);
        }

        // Priority 4: Go to known Truffle location
        if let Some((pos, cell)) = self
            .grid
            .iter()
            .find(|(_, cell)| cell.item == Some(Truffle))
        {
            let pos = Pos::from((pos.0 + 1, pos.1));
            self.ctx.debug(format!(
                "Going to truffle at position: {}. Last observed {}, {} ticks \
                 ago",
                pos,
                cell.last_observed,
                tick - cell.last_observed
            ));
            return Action::MoveTo(Pos::from(pos));
        }

        // Priority 5: Explore random unknown cells
        let unknown_cells: Vec<_> = self
            .grid
            .iter()
            .filter(|(_, cell)| cell.kind == CellKind::Unknown)
            .collect();
            
        if !unknown_cells.is_empty() {
            let random_index = self.rng.random_range(0..unknown_cells.len());
            let (pos, _) = unknown_cells[random_index];
            
            self.ctx.debug(format!(
                "Found random unexplored cell at position: {}",
                Pos::from(pos)
            ));
            return Action::MoveTo(Pos::from(pos));
        }

        // Priority 6: Random movement if nothing else to do
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
            .debug("No unexplored cells found, moving to default dir");
        Action::MoveDir(self.default_dir)
    }
}

impl BotUpdate for CrumbFollower {
    fn update(&mut self, update: ServerUpdate) -> Option<BotResponse> {
        let radar = &update.radar;
        let action_result = update.action_result;
        if let Some(result) = action_result {
            self.ctx.debug(format!("{result:?}"));

            if ActionStatusDiscriminants::InProgress == (&result.status).into()
            {
                self.ctx
                    .info("Previous action still in progress, waiting...");
                return None;
            }
        }

        // Determine the next action using a linear decision flow
        let action = self.determine_next_action(radar, update.tick);

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
        let (map_w, map_h) = map_size();
        Self {
            grid: GridWorld::new(map_w, map_h, ClientCellState::default()),
            rng: SmallRng::seed_from_u64(ctx.bot_id as u64),
            ctx,
            default_dir: Dir::Up,
            action_counter: 0,
            seen_bots: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<()> {
        crate::run_loop(self)
    }
}
