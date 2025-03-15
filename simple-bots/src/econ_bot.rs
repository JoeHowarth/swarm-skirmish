use std::fmt;

use rand::{rngs::SmallRng, Rng, SeedableRng};
use strum::IntoDiscriminant;
use swarm_lib::{
    bot_logger::BotLogger,
    gridworld::GridWorld,
    known_map::{ClientBotData, ClientCellState},
    Action,
    ActionWithId,
    Bot,
    BotData,
    BotUpdate,
    BuildingKind,
    CellKind,
    CellStateRadar,
    DecisionResult::{self, Act, Continue, Wait},
    Dir,
    FrameKind,
    Item::{self, *},
    NewBotNoMangeFn,
    Pos,
    RadarData,
    Subsystem,
    Subsystems,
};

macro_rules! debug {
    ($self:expr, $($arg:tt)*) => {
        $self.ctx.debug(format!($($arg)*))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum EconBotRole {
    #[default]
    None,
    Base,
    Gatherer(GathererState),
    Explorer(ExplorerState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GathererState {
    Idle,
    Gathering { base: Pos },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorerState {
    Idle,
    Exploring { base: Pos },
}

pub struct EconBot {
    pub ctx: BotLogger,
    pub rng: SmallRng,
    pub action_counter: u32,
    pub grid: GridWorld<ClientCellState>,
    pub seen_bots: Vec<ClientBotData>,

    pub role: EconBotRole,
}

impl Bot for EconBot {
    fn update(&mut self, update: BotUpdate) -> Option<ActionWithId> {
        self.ctx.set_tick(update.tick);
        self.ctx.log_debug_info(&update, 5);

        if self.role == EconBotRole::None {
            self.role = Self::determine_role(&update.bot_data);
            debug!(self, "Determined role: {:?}", self.role);
        }

        debug!(
            self,
            "Inventory: {:?}, Energy: {}",
            update.bot_data.inventory,
            update.bot_data.energy.0
        );

        let mut role = std::mem::take(&mut self.role);
        let action = match &mut role {
            EconBotRole::None => unreachable!(),
            EconBotRole::Base => self.base_behavior(&update),
            EconBotRole::Gatherer(state) => {
                self.gatherer_behavior(state, &update)
            }
            EconBotRole::Explorer(state) => {
                self.explorer_behavior(state, &update)
            }
        };
        self.role = role;

        self.ctx.flush_buffer_to_stdout();

        // Enrich action with id
        match action {
            Act(action) => {
                self.action_counter += 1;
                debug!(
                    self,
                    "Taking action: {:?} with id: {}",
                    action,
                    self.action_counter
                );
                Some(ActionWithId {
                    id: self.action_counter,
                    action,
                })
            }
            Wait => None,
            Continue => None,
        }
    }
}

impl EconBot {
    fn determine_role(bot_data: &BotData) -> EconBotRole {
        if matches!(bot_data.frame, FrameKind::Building(_)) {
            return EconBotRole::Base;
        }

        if bot_data.subsystems.has(Subsystem::CargoBay) {
            return EconBotRole::Gatherer(GathererState::Idle);
        }

        EconBotRole::Explorer(ExplorerState::Idle)
    }

    fn base_behavior(&mut self, update: &BotUpdate) -> DecisionResult {
        self.wait_for_in_progress_action(&update.in_progress_action)?;

        let bot_data = &update.bot_data;
        debug!(
            self,
            "Base checking inventory - Metal: {}, Energy: {}",
            bot_data.inventory.get(Item::Metal),
            bot_data.energy.0
        );

        let tractor_count = self
            .seen_bots
            .iter()
            .filter(|b| b.frame == FrameKind::Tractor)
            .count();

        debug!(self, "Current tractor count: {}", tractor_count);

        if tractor_count == 0 {
            if bot_data.inventory.get(Item::Metal)
                >= FrameKind::Tractor.build_cost()
            {
                debug!(
                    self,
                    "Base has enough metal ({}), building a tractor with \
                     cargo bay",
                    bot_data.inventory.get(Item::Metal)
                );
                return Act(Action::Build(
                    Dir::Up,
                    FrameKind::Tractor,
                    Subsystems::new([(Subsystem::CargoBay, 6)]),
                ));
            }
        }

        let generator_count = self
            .seen_bots
            .iter()
            .filter(|b| b.subsystems.has(Subsystem::Generator))
            .count();

        debug!(self, "Current generator count: {}", generator_count);

        if generator_count == 0 {
            if bot_data.inventory.get(Item::Metal)
                >= FrameKind::Building(BuildingKind::Small).build_cost()
            {
                debug!(
                    self,
                    "Base has enough metal ({}), building a small building \
                     with generator",
                    bot_data.inventory.get(Item::Metal)
                );
                return Act(Action::Build(
                    Dir::Right,
                    FrameKind::Building(BuildingKind::Small),
                    Subsystems::new([
                        (Subsystem::Generator, 1),
                        (Subsystem::PowerCell, 5),
                    ]),
                ));
            }
        }

        if bot_data.inventory.get(Item::Metal)
            >= FrameKind::Tractor.build_cost()
        {
            debug!(
                self,
                "Base has enough metal ({}), building a tractor with cargo \
                 bay and power cell",
                bot_data.inventory.get(Item::Metal)
            );
            return Act(Action::Build(
                Dir::Up,
                FrameKind::Tractor,
                Subsystems::new([
                    (Subsystem::CargoBay, 5),
                    (Subsystem::PowerCell, 1),
                ]),
            ));
        }

        debug!(
            self,
            "Base waiting for resources, current metal: {}",
            bot_data.inventory.get(Item::Metal)
        );
        Wait
    }

    fn gatherer_behavior(
        &mut self,
        state: &mut GathererState,
        update: &BotUpdate,
    ) -> DecisionResult {
        let bot = &update.bot_data;
        match state {
            GathererState::Idle => {
                debug!(self, "Gatherer is idle, looking for a base");

                // let (target, _) = self
                //     .grid
                //     .iter()
                //     .find(|(pos, cell)| {
                //         if cell.is_unknown() {
                //             return false;
                //         }

                //         println!("Cell Known {pos:?}");
                //         let Some(pawn_id) = cell.pawn else {
                //             return false;
                //         };

                //         let pawn = self
                //             .seen_bots
                //             .iter()
                //             .find(|b| b.bot_id == pawn_id)
                //             .expect("Grid pawn not found in seen bots");

                //         println!(
                //             "Cell has pawn {:?} with frame {:?}",
                //             pawn.bot_id, pawn.frame
                //         );
                //         pawn.frame ==
                // FrameKind::Building(BuildingKind::Small)
                //     })
                //     .unwrap();
                // println!("Target {target:?}");

                let Some((target, _)) =
                    self.grid.find_nearby(bot.pos, 1000, |cell| {
                        let Some(pawn_id) = cell.pawn else {
                            return false;
                        };

                        let pawn = self
                            .seen_bots
                            .iter()
                            .find(|b| b.bot_id == pawn_id)
                            .expect("Grid pawn not found in seen bots");

                        println!(
                            "Cell has pawn {:?} with frame {:?}",
                            pawn.bot_id, pawn.frame
                        );
                        pawn.frame == FrameKind::Building(BuildingKind::Small)
                    })
                else {
                    debug!(self, "No base found, waiting");
                    return Wait;
                };
                debug!(self, "Found base at {:?}", target);
                *state = GathererState::Gathering { base: target };
                return self.gatherer_behavior(state, update);
            }
            GathererState::Gathering { base } => {
                self.ensure_energy(bot)?;

                self.wait_for_in_progress_action(&update.in_progress_action)?;

                self.gather(bot, base)?;

                self.explore(bot, 1000)
            }
        }
    }

    fn gather(&mut self, bot: &BotData, base: &Pos) -> DecisionResult {
        // next to base and has metal => Transfer
        if bot.inventory.has(Item::Metal) && bot.pos.is_adjacent(base) {
            let dir = bot.pos.dir_to(base).unwrap();
            debug!(
                self,
                "Next to base with metal, transferring in direction: {:?}", dir
            );
            return Act(Action::Transfer((Item::Metal, dir)));
        }

        // full => return to base to unload
        if bot.inventory.size() == bot.inventory.capacity {
            debug!(
                self,
                "Inventory full ({}/{}), returning to base",
                bot.inventory.size(),
                bot.inventory.capacity
            );
            if let Some(path) = self.grid.find_path_adj(bot.pos, *base) {
                return Act(Action::MoveTo(path));
            }
        }

        // next to metal => pick up
        // metal nearby => move to
        if let Some((target, _)) = self
            .grid
            .find_nearby(bot.pos, 50, |cell| cell.item == Some(Item::Metal))
        {
            debug!(self, "Found metal at {:?}", target);
            return self.move_and_act(bot, target, |dir| {
                Action::Pickup((Item::Metal, Some(dir)))
            });
        }

        // Check nearby if there are any unexplored cells that may have metal
        // before going back to base
        self.explore(bot, 10)?;

        // has metal => return to base to unload
        if bot.inventory.has(Item::Metal) {
            debug!(
                self,
                "Has metal ({}), returning to base",
                bot.inventory.get(Item::Metal)
            );
            if let Some(path) = self.grid.find_path_adj(bot.pos, *base) {
                return Act(Action::MoveTo(path));
            }
        }
        Continue
    }

    fn explorer_behavior(
        &mut self,
        state: &mut ExplorerState,
        update: &BotUpdate,
    ) -> DecisionResult {
        let bot = &update.bot_data;
        match state {
            ExplorerState::Idle => {
                debug!(self, "Explorer is idle");
                Wait
            }
            ExplorerState::Exploring { base } => {
                self.ensure_energy(bot)?;
                self.wait_for_in_progress_action(&update.in_progress_action)?;
                self.explore(bot, 1000)?;

                // Return to base
                let distance = bot.pos.manhattan_distance(base);
                if distance > 10 {
                    debug!(
                        self,
                        "Explorer has nothing to do, returning to base ",
                    );
                    if let Some(path) = self.grid.find_path_adj(bot.pos, *base)
                    {
                        return Act(Action::MoveTo(path));
                    }
                }
                Wait
            }
        }
    }

    fn explore(
        &mut self,
        bot: &BotData,
        max_distance: usize,
    ) -> DecisionResult {
        let unknown_cell =
            self.grid
                .find_nearby(bot.pos, max_distance, |cell| cell.is_unknown());

        if let Some((target, _)) = unknown_cell {
            debug!(self, "Found unknown cell at {:?}", target);
            if let Some(path) = self.grid.find_path(bot.pos, target) {
                return Act(Action::MoveTo(path));
            }
        } else {
            debug!(self, "No unknown cells found within range");
        }

        Continue
    }

    fn ensure_energy(&mut self, bot: &BotData) -> DecisionResult {
        let base = match self
            .find_bot(bot.pos, 1000, |b| b.subsystems.has(Subsystem::Generator))
        {
            Some((base, _)) => base,
            None => {
                let Some((base, _)) = self.find_bot(bot.pos, 1000, |b| {
                    b.frame == FrameKind::Building(BuildingKind::Small)
                }) else {
                    debug!(self, "No recharge base found, continuing");
                    return Continue;
                };
                base
            }
        };

        let energy_level = bot.energy.0;
        let max_energy = bot.max_energy().0;
        let distance_to_base = bot.pos.manhattan_distance(&base);

        if energy_level < max_energy {
            if let Some(dir) = bot.pos.dir_to(&base) {
                debug!(
                    self,
                    "Adjacent to base, recharging. Energy: {}/{}",
                    energy_level,
                    max_energy
                );
                return Act(Action::Recharge(dir));
            }

            if energy_level < 50
                || distance_to_base + 10 > energy_level as usize
            {
                debug!(
                    self,
                    "Low energy: {}/{}, distance to base: {}, moving to base",
                    energy_level,
                    max_energy,
                    distance_to_base
                );
                return self.move_and_act(bot, base, Action::Recharge);
            }
        }

        Continue
    }

    fn move_and_act(
        &mut self,
        bot: &BotData,
        target: Pos,
        action: impl FnOnce(Dir) -> Action,
    ) -> DecisionResult {
        if let Some(dir) = bot.pos.dir_to(&target) {
            debug!(
                self,
                "Adjacent to target {:?}, taking action in direction: {:?}",
                target,
                dir
            );
            return Act(action(dir));
        }
        if let Some(path) = self.grid.find_path_adj(bot.pos, target) {
            debug!(
                self,
                "Moving to target {:?}, path length: {}",
                target,
                path.len()
            );
            return Act(Action::MoveTo(path));
        }

        debug!(self, "Cannot find path to target {:?}", target);
        Continue
    }

    fn wait_for_in_progress_action(
        &mut self,
        in_progress_action: &Option<ActionWithId>,
    ) -> DecisionResult {
        let Some(in_progress_action) = &in_progress_action else {
            return Continue;
        };
        debug!(
            self,
            "Waiting for in_progress_action to complete: {:?} {:?}",
            in_progress_action.id,
            in_progress_action.action.discriminant()
        );
        return Wait;
    }

    fn find_bot(
        &self,
        pos: Pos,
        max_distance: usize,
        pred: impl Fn(&ClientBotData) -> bool,
    ) -> Option<(Pos, &ClientCellState)> {
        self.grid.find_nearby(pos, max_distance, |cell| {
            //
            let Some(pawn_id) = cell.pawn else {
                return false;
            };
            let Some(pawn) =
                self.seen_bots.iter().find(|b| b.bot_id == pawn_id)
            else {
                return false;
            };
            pred(pawn)
        })
    }
}

impl fmt::Debug for EconBot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EconBot")
            .field("action_counter", &self.action_counter)
            .field("role", &self.role)
            .field("grid_size", &(self.grid.width(), self.grid.height()))
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
        ctx.debug("No in-progress action, continuing".to_string());
        return Continue;
    };

    let Action::MoveTo(path) = &in_progress_action.action else {
        ctx.debug("In-progress action is not MoveTo, continuing".to_string());
        return Continue;
    };

    if path.contains(&pos) {
        ctx.debug(format!(
            "Waiting for in_progress_action to complete: \
             {in_progress_action:?}"
        ));
        return Wait;
    }

    ctx.debug(format!(
        "Target position {:?} is not in current path, continuing",
        pos
    ));
    Continue
}
