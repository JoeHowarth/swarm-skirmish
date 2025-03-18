
use rand::rngs::SmallRng;
use strum::IntoDiscriminant;
use swarm_lib::{
    bot_logger::BotLogger,
    known_map::ClientBotData,
    Action,
    ActionWithId,
    Bot,
    BotData,
    BotUpdate,
    BuildingKind,
    DecisionResult::{self, Act, Continue, Wait},
    Dir,
    FrameKind,
    Item::{self},
    Pos,
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
    // pub grid: GridWorld<ClientCellState>,
    // pub seen_bots: Vec<ClientBotData>,
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
            Act(action, reason) => {
                self.action_counter += 1;
                debug!(
                    self,
                    "Taking action: {:?} with id: {}, reason: {}",
                    action,
                    self.action_counter,
                    reason
                );
                Some(ActionWithId {
                    id: self.action_counter,
                    action,
                    reason,
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

        let bot = &update.bot_data;
        debug!(
            self,
            "Base checking inventory - Metal: {}, Energy: {}",
            bot.inventory.get(Item::Metal),
            bot.energy.0
        );

        if bot.energy.0 < 10 && !bot.subsystems.has(Subsystem::Generator) {
            if let Some((pos, _)) = self
                .find_bot(bot, 1, |b| b.subsystems.has(Subsystem::Generator))
            {
                let dir = bot.pos.dir_to(&pos).unwrap();
                return Act(Action::Recharge(dir), "Recharging");
            }
        }

        let tractor_count = bot
            .known_bots
            .iter()
            .filter(|b| b.frame == FrameKind::Tractor)
            .count();
        let flea_count = bot
            .known_bots
            .iter()
            .filter(|b| b.frame == FrameKind::Flea)
            .count();

        debug!(self, "Current tractor count: {}", tractor_count);

        if flea_count == 0 {
            if bot.inventory.get(Item::Metal) >= FrameKind::Flea.build_cost() {
                debug!(
                    self,
                    "Base has enough metal ({}), building a flea with cargo \
                     bay",
                    bot.inventory.get(Item::Metal)
                );
                return Act(
                    Action::Build(
                        Dir::Up,
                        FrameKind::Flea,
                        Subsystems::new([(Subsystem::CargoBay, 1)]),
                    ),
                    "Building Flea-Harvester",
                );
            } else {
                return Wait;
            }
        }

        let generator_count = bot
            .known_bots
            .iter()
            .filter(|b| b.subsystems.has(Subsystem::Generator))
            .count();

        debug!(self, "Current generator count: {}", generator_count);

        if generator_count == 0 {
            if bot.inventory.get(Item::Metal)
                >= FrameKind::Building(BuildingKind::Small).build_cost()
            {
                debug!(
                    self,
                    "Base has enough metal ({}), building a small building \
                     with generator",
                    bot.inventory.get(Item::Metal)
                );
                return Act(
                    Action::Build(
                        Dir::Right,
                        FrameKind::Building(BuildingKind::Small),
                        Subsystems::new([
                            (Subsystem::Generator, 1),
                            (Subsystem::PowerCell, 5),
                        ]),
                    ),
                    "Building generator",
                );
            } else {
                return Wait;
            }
        }

        if bot.inventory.get(Item::Metal) >= FrameKind::Tractor.build_cost() {
            debug!(
                self,
                "Base has enough metal ({}), building a tractor with cargo \
                 bay and power cell",
                bot.inventory.get(Item::Metal)
            );
            return Act(
                Action::Build(
                    Dir::Up,
                    FrameKind::Tractor,
                    Subsystems::new([
                        (Subsystem::CargoBay, 4),
                        (Subsystem::PowerCell, 2),
                    ]),
                ),
                "Building Tractor-Harvester ",
            );
        }

        debug!(
            self,
            "Base waiting for resources, current metal: {}",
            bot.inventory.get(Item::Metal)
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

                let Some((target, _)) =
                    bot.known_map.find_nearby(bot.pos, 1000, |cell| {
                        let Some(pawn_id) = cell.pawn else {
                            return false;
                        };

                        let pawn = bot
                            .known_bots
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
                    debug!(self, "In idle state, no base found, waiting");
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
            return Act(
                Action::Transfer((Item::Metal, dir)),
                "Transferring metal",
            );
        }

        // full => return to base to unload
        if bot.inventory.size() == bot.inventory.capacity {
            debug!(
                self,
                "Inventory full ({}/{}), returning to base",
                bot.inventory.size(),
                bot.inventory.capacity
            );
            if let Some(path) = bot.known_map.find_path_adj(bot.pos, *base) {
                return Act(Action::MoveTo(path), "Moving to base");
            }
        }

        // next to metal => pick up
        // metal nearby => move to
        if let Some((target, _)) =
            bot.known_map
                .find_nearby(bot.pos, 50, |cell| cell.item == Some(Item::Metal))
        {
            debug!(self, "Found metal at {:?}", target);
            return self.move_and_act(
                bot,
                target,
                |dir| Action::Pickup((Item::Metal, Some(dir))),
                "Picking up metal",
            );
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
            if let Some(path) = bot.known_map.find_path_adj(bot.pos, *base) {
                return Act(Action::MoveTo(path), "Delivering metal to base");
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
                    if let Some(path) =
                        bot.known_map.find_path_adj(bot.pos, *base)
                    {
                        return Act(
                            Action::MoveTo(path),
                            "Nothing to explore, returning to base",
                        );
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
            bot.known_map
                .find_nearby(bot.pos, max_distance, |cell| cell.is_unknown());

        if let Some((target, _)) = unknown_cell {
            debug!(self, "Found unknown cell at {:?}", target);
            if let Some(path) = bot.known_map.find_path(bot.pos, target) {
                return Act(Action::MoveTo(path), "Exploring");
            }
        } else {
            debug!(self, "No unknown cells found within range");
        }

        Continue
    }

    fn ensure_energy(&mut self, bot: &BotData) -> DecisionResult {
        let base = match self
            .find_bot(bot, 1000, |b| b.subsystems.has(Subsystem::Generator))
        {
            Some((base, _)) => base,
            None => {
                let Some((base, _)) = self.find_bot(bot, 1000, |b| {
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
                if bot.max_energy().0 - 10 >= energy_level {
                    return Act(Action::Recharge(dir), "Recharging");
                }
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
                return self.move_and_act(
                    bot,
                    base,
                    Action::Recharge,
                    "Returning to base to recharge",
                );
            }
        }

        Continue
    }

    fn move_and_act(
        &mut self,
        bot: &BotData,
        target: Pos,
        action: impl FnOnce(Dir) -> Action,
        reason: &'static str,
    ) -> DecisionResult {
        if let Some(dir) = bot.pos.dir_to(&target) {
            debug!(
                self,
                "Adjacent to target {:?}, taking action in direction: {:?}",
                target,
                dir
            );
            return Act(action(dir), reason);
        }
        if let Some(path) = bot.known_map.find_path_adj(bot.pos, target) {
            debug!(
                self,
                "Moving to target {:?}, path length: {}",
                target,
                path.len()
            );
            return Act(Action::MoveTo(path), reason);
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

    fn find_bot<'a>(
        &'a self,
        bot: &'a BotData,
        max_distance: usize,
        pred: impl Fn(&'a ClientBotData) -> bool,
    ) -> Option<(Pos, &'a ClientBotData)> {
        let mut found = None;
        bot.known_map
            .find_nearby(bot.pos, max_distance, |cell| {
                //
                let Some(pawn_id) = cell.pawn else {
                    return false;
                };
                let Some(pawn) =
                    bot.known_bots.iter().find(|b| b.bot_id == pawn_id)
                else {
                    return false;
                };
                if pred(pawn) {
                    found = Some(pawn);
                    true
                } else {
                    false
                }
            })
            .map(|(pos, _)| (pos, found.unwrap()))
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
