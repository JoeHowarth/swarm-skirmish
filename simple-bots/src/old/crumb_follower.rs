use rand::{rngs::SmallRng, Rng};
use swarm_lib::{
    bot_logger::{BotLogger, LogEntry},
    gridworld::PassableCell,
    known_map::{ClientBotData, KnownMap},
    Action,
    ActionWithId,
    Bot,
    BotUpdate,
    CellStateRadar,
    Dir,
    Item::*,
    Pos,
};

pub struct CrumbFollower {
    pub ctx: BotLogger,
    pub rng: SmallRng,
    pub default_dir: Dir,
    pub action_counter: u32,
    // pub grid: GridWorld<ClientCellState>,
    pub seen_bots: Vec<ClientBotData>,
}

use std::fmt;

impl fmt::Debug for CrumbFollower {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CrumbFollower")
            .field("action_counter", &self.action_counter)
            .field("default_dir", &self.default_dir)
            .field("seen_bots_count", &self.seen_bots.len())
            .finish()
    }
}

impl CrumbFollower {
    // Helper method to determine the next action with a linear flow instead of
    // nested conditionals
    fn determine_next_action(
        &mut self,
        curr_pos: Pos,
        map: &KnownMap,
        tick: u32,
    ) -> (Action, &'static str) {
        // Go to known Fent location
        if let Some((pos, cell)) =
            map.iter().find(|(_, cell)| cell.item == Some(Fent))
        {
            let pos = Pos::from((pos.0, pos.1));

            if let Some(path) = map.find_path(curr_pos, pos) {
                self.ctx.debug(format!(
                    "Going to Fent at position: {}. Last observed {}, {} \
                     ticks ago, current pos: {}",
                    pos,
                    cell.last_observed,
                    tick - cell.last_observed,
                    curr_pos
                ));
                return (Action::MoveTo(path), "Going to Fent");
            }

            self.ctx.debug("No path to Fent");
        }

        // Harvest nearby Truffle
        if let Some(dir) =
            map.find_adj(curr_pos, CellStateRadar::has_item(Truffle))
        {
            self.ctx.debug(format!("Harvesting Truffle {dir:?}"));
            return (Action::Harvest(dir), "Harvesting Truffle");
        }

        // Go to known Truffle location
        if let Some((pos, cell)) =
            map.find_nearby(curr_pos, 1000, |cell| cell.item == Some(Truffle))
        {
            self.ctx.debug(format!(
                "Going to truffle at position: {}. Last observed {}, {} ticks \
                 ago, current pos: {}",
                pos,
                cell.last_observed,
                tick - cell.last_observed,
                curr_pos
            ));

            if let Some(path) = map.find_path_adj(curr_pos, pos) {
                return (Action::MoveTo(path), "Going to truffle");
            }

            self.ctx.debug("No path to truffle");
        }

        // Follow Crumb
        if let Some((pos, _)) =
            map.find_nearby(curr_pos, 1000, CellStateRadar::has_item(Crumb))
        {
            self.ctx
                .debug(format!("Found Crumb at world position: {}", pos));

            if let Some(path) = map.find_path(curr_pos, pos) {
                return (Action::MoveTo(path), "Following crumbs");
            }

            self.ctx.debug("No path to crumb");
        }

        // Explore random unknown cells
        let unknown_cells =
            map.find_nearby(curr_pos, 1000, |cell| cell.is_unknown());

        if let Some((pos, _)) = unknown_cells {
            self.ctx.debug(format!(
                "Found random unexplored cell at position: {}",
                Pos::from(pos)
            ));

            if let Some(path) = map.find_path(curr_pos, pos) {
                return (Action::MoveTo(path), "Exploring unknown cell");
            }

            self.ctx.debug("No path to random unexplored cell");
        }

        // Random movement if nothing else to do
        let default_dir_blocked = (curr_pos + self.default_dir)
            .and_then(|p| map.try_get(p))
            .map(|c| c.is_blocked())
            .unwrap_or(true);

        if self.rng.random_bool(0.2) || default_dir_blocked {
            self.ctx.debug("Changing default dir");
            self.default_dir =
                Dir::from_repr(self.rng.random_range(0..=3)).unwrap();
        }

        self.ctx
            .debug("No unexplored cells found, moving to default dir");
        (Action::MoveDir(self.default_dir), "Exploring randomly")
    }
}

impl Bot for CrumbFollower {
    fn update(
        &mut self,
        update: BotUpdate,
    ) -> (Option<ActionWithId>, Vec<LogEntry>) {
        self.ctx.set_tick(update.tick);
        self.ctx.log_debug_info(&update, 1);

        if let Some(action) = update.in_progress_action {
            self.ctx.debug(format!(
                "Previous action still in progress, waiting... Action: \
                 {action:?}"
            ));
            return (None, Vec::new());
        }

        // Determine the next action using a linear decision flow
        let (action, reason) = self.determine_next_action(
            update.bot_data.pos,
            &update.bot_data.known_map,
            update.tick,
        );

        let logs = self.ctx.flush_buffer_to_stdout();

        // Build and send response with movement action
        self.action_counter += 1;
        (
            Some(ActionWithId {
                id: self.action_counter,
                action,
                reason,
            }),
            logs,
        )
    }
}
