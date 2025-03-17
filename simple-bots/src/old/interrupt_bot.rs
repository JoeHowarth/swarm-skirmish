use std::fmt;

use rand::{rngs::SmallRng, Rng, SeedableRng};
use swarm_lib::{
    bot_logger::BotLogger,
    gridworld::GridWorld,
    known_map::{ClientBotData, ClientCellState, KnownMap},
    Action,
    ActionWithId,
    Bot,
    BotUpdate,
    CellKind,
    CellStateRadar,
    DecisionResult::{self, Act, Continue, Wait},
    Dir,
    Item::{self, *},
    NewBotNoMangeFn,
    Pos,
    RadarData,
};

pub struct InterruptBot {
    pub ctx: BotLogger,
    pub rng: SmallRng,
    pub default_dir: Dir,
    pub action_counter: u32,
    // pub grid: GridWorld<ClientCellState>,
    pub seen_bots: Vec<ClientBotData>,
}

macro_rules! debug {
    ($self:expr, $($arg:tt)*) => {
        $self.ctx.debug(format!($($arg)*))
    }
}

impl InterruptBot {
    // Helper method to determine the next action with a linear flow instead of
    // nested conditionals
    fn determine_next_action(
        &mut self,
        curr_pos: Pos,
        known_map: &KnownMap,
        in_progress_action: Option<ActionWithId>,
    ) -> DecisionResult {
        self.go_to_fent_if_seen(curr_pos, known_map, &in_progress_action)?;

        self.seek_and_pickup_truffle(curr_pos, &in_progress_action, known_map)?;

        // If we have an in progress action, wait for it to complete
        if let Some(action) = in_progress_action {
            debug!(
                self,
                "No higher priority action, waiting for in_progress_action to \
                 complete: {action:?}"
            );

            self.ctx.debug(format!(
                "No higher priority action, waiting for in_progress_action to \
                 complete: {action:?}"
            ));
            return Wait;
        }

        self.follow_crumbs(curr_pos, known_map)?;

        // Explore random unknown cells
        self.explore_unknown_cells(curr_pos, known_map)
    }

    fn go_to_fent_if_seen(
        &mut self,
        curr_pos: Pos,
        known_map: &KnownMap,
        in_progress_action: &Option<ActionWithId>,
    ) -> DecisionResult {
        if let Some((pos, _cell)) =
            known_map.iter().find(|(_, cell)| cell.item == Some(Fent))
        {
            let pos = Pos::from((pos.0, pos.1));
            existing_path_contains_pos(in_progress_action, pos, &mut self.ctx)?;

            if let Some(path) = known_map.find_path(curr_pos, pos) {
                debug!(self, "Going to Fent at position: {pos}");

                return Act(Action::MoveTo(path), "Going to Fent");
            }

            debug!(self, "No path to Fent");
        }

        Continue
    }

    fn seek_and_pickup_truffle(
        &mut self,
        curr_pos: Pos,
        in_progress_action: &Option<ActionWithId>,
        radar: &KnownMap,
    ) -> DecisionResult {
        // Harvest nearby Truffle
        if let Some(dir) =
            radar.find_adj(curr_pos, |cell| cell.item == Some(Truffle))
        {
            debug!(self, "Picking up Truffle {dir:?}");
            return Act(Action::Pickup((Truffle, Some(dir))), "Picking up Truffle");
        }

        // Go to known Truffle location
        if let Some((pos, _cell)) =
            radar.iter().find(|(_, cell)| cell.item == Some(Truffle))
        {
            let pos = Pos::from((pos.0 + 1, pos.1));
            existing_path_contains_pos(in_progress_action, pos, &mut self.ctx)?;

            debug!(self, "Going to truffle at position: {pos}");

            if let Some(path) = radar.find_path(curr_pos, pos) {
                return Act(Action::MoveTo(path), "Going to truffle");
            }

            debug!(self, "No path to truffle");
        }

        Continue
    }

    fn follow_crumbs(
        &mut self,
        curr_pos: Pos,
        known_map: &KnownMap,
    ) -> DecisionResult {
        let Some((pos, _)) = known_map.find_nearby(
            curr_pos,
            1000,
            CellStateRadar::has_item(Crumb),
        ) else {
            return Continue;
        };

        debug!(self, "Found Crumb at world position: {pos}");

        if let Some(path) = known_map.find_path(curr_pos, pos) {
            return Act(Action::MoveTo(path), "Following crumbs");
        }

        debug!(self, "No path to crumb");

        Continue
    }

    fn explore_unknown_cells(
        &mut self,
        curr_pos: Pos,
        radar: &KnownMap,
    ) -> DecisionResult {
        let unknown_cells: Vec<_> = radar
            .iter()
            .filter(|(_, cell)| cell.kind == CellKind::Unknown)
            .collect();

        if !unknown_cells.is_empty() {
            let random_index = self.rng.random_range(0..unknown_cells.len());
            let (pos, _) = unknown_cells[random_index];

            debug!(self, "Found random unexplored cell at position: {pos:?}");

            if let Some(path) = radar.find_path(curr_pos, pos) {
                return Act(Action::MoveTo(path), "Exploring unknown cell");
            }

            debug!(self, "No path to random unexplored cell");
        }

        debug!(self, "No unexplored cells found, moving exploring randomly");

        // Random movement if nothing else to do
        let default_dir_blocked = (curr_pos + self.default_dir)
            .and_then(|p| radar.try_get(p))
            .map(|c| c.kind)
            .unwrap_or(CellKind::Blocked)
            == CellKind::Blocked;

        if self.rng.random_bool(0.2) || default_dir_blocked {
            debug!(self, "Changing default dir");
            self.default_dir =
                Dir::from_repr(self.rng.random_range(0..=3)).unwrap();
        }

        Act(Action::MoveDir(self.default_dir), "Exploring randomly")
    }
}

impl Bot for InterruptBot {
    fn update(&mut self, update: BotUpdate) -> Option<ActionWithId> {
        self.ctx.set_tick(update.tick);
        self.ctx.log_debug_info(&update, 1);

        // Determine the next action using a linear decision flow
        let action = self.determine_next_action(
            update.bot_data.pos,
            &update.bot_data.known_map,
            update.in_progress_action,
        );

        self.ctx.flush_buffer_to_stdout();

        // Enrich action with id
        self.action_counter += 1;
        match action {
            Act(action, reason) => Some(ActionWithId {
                id: self.action_counter,
                action,
                reason,
            }),
            Wait => None,
            Continue => None,
        }
    }
}

impl fmt::Debug for InterruptBot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InterruptBot")
            .field("action_counter", &self.action_counter)
            .field("default_dir", &self.default_dir)
            .field("seen_bots_count", &self.seen_bots.len())
            .finish()
    }
}

fn existing_path_contains_pos(
    in_progress_action: &Option<ActionWithId>,
    pos: Pos,
    ctx: &mut BotLogger,
) -> DecisionResult {
    let Some(in_progress_action) = &in_progress_action else {
        return Continue;
    };

    let Action::MoveTo(path) = &in_progress_action.action else {
        return Continue;
    };

    if path.contains(&pos) {
        ctx.debug(format!(
            "Waiting for in_progress_action to complete: \
             {in_progress_action:?}"
        ));
        return Wait;
    }

    Continue
}
