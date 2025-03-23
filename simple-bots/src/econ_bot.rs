use std::collections::HashMap;

use rand::rngs::SmallRng;
use strum::IntoDiscriminant;
use swarm_lib::{
    bot_logger::{BotLogger, LogEntry},
    gridworld::PassableCell,
    known_map::{ClientBotData, KnownMap},
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

macro_rules! info {
    ($self:expr, $($arg:tt)*) => {
        $self.ctx.info(format!($($arg)*))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum EconBotRole {
    #[default]
    None,
    Base(BaseState),
    Gatherer(GathererState),
    Explorer(ExplorerState),
}

impl EconBotRole {
    // /// Returns a mutable reference to the base state.
    // /// Panics if the role is not Base.
    // pub fn base(&mut self) -> &mut BaseState {
    //     match self {
    //         EconBotRole::Base(Some(state)) => state,
    //         _ => unreachable!(),
    //     }
    // }

    // /// Returns a mutable reference to the gatherer state.
    // /// Panics if the role is not Gatherer.
    // pub fn gatherer(&mut self) -> &mut GathererState {
    //     match self {
    //         EconBotRole::Gatherer(Some(state)) => state,
    //         _ => unreachable!(),
    //     }
    // }

    // /// Returns a mutable reference to the explorer state.
    // /// Panics if the role is not Explorer.
    // pub fn explorer(&mut self) -> &mut ExplorerState {
    //     match self {
    //         EconBotRole::Explorer(Some(state)) => state,
    //         _ => unreachable!(),
    //     }
    // }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseState {
    /// Map of bot id to tick of last shared map
    pub last_shared_map: HashMap<u32, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GathererState {
    pub last_shared_map_tick: u32,
    pub base: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerState {
    pub last_shared_map_tick: u32,
    pub base: Pos,
}

pub struct EconBot {
    pub ctx: BotLogger,
    pub rng: SmallRng,
    pub action_counter: u32,
    pub role: EconBotRole,
}

impl Bot for EconBot {
    fn update(
        &mut self,
        update: BotUpdate,
    ) -> (Option<ActionWithId>, Vec<LogEntry>) {
        self.ctx.set_tick(update.tick);
        self.ctx.log_debug_info(&update, 5);

        if self.role == EconBotRole::None {
            self.role = Self::determine_role(&update.bot_data, &update);
            info!(self, "Determined role: {:?}", self.role);
        }

        info!(
            self,
            "Inventory: {:?}, Energy: {}",
            update.bot_data.inventory,
            update.bot_data.energy.0
        );

        let mut role = std::mem::take(&mut self.role);
        let decision = match &mut role {
            EconBotRole::None => unreachable!(),
            EconBotRole::Base(state) => self.base_behavior(state, &update),
            EconBotRole::Gatherer(state) => {
                self.gatherer_behavior(state, &update)
            }
            EconBotRole::Explorer(state) => {
                self.explorer_behavior(state, &update)
            }
        };
        self.role = role;

        // Enrich action with id
        let action = match decision {
            Act(action, reason) => {
                self.action_counter += 1;
                info!(
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
        };

        let logs = self.ctx.flush_buffer_to_stdout();
        (action, logs)
    }
}

impl EconBot {
    fn determine_role(bot_data: &BotData, update: &BotUpdate) -> EconBotRole {
        if matches!(bot_data.frame, FrameKind::Building(_)) {
            return EconBotRole::Base(Self::init_base_state(update));
        }

        if bot_data.subsystems.has(Subsystem::CargoBay) {
            return EconBotRole::Gatherer(Self::init_gatherer_state(update));
        }

        EconBotRole::Explorer(Self::init_explorer_state(update))
    }

    fn init_base_state(_update: &BotUpdate) -> BaseState {
        BaseState {
            last_shared_map: HashMap::new(),
        }
    }

    fn base_behavior(
        &mut self,
        state: &mut BaseState,
        update: &BotUpdate,
    ) -> DecisionResult {
        self.wait_for_in_progress_action(&update.in_progress_action)?;

        let bot = &update.bot_data;
        info!(
            self,
            "Base checking inventory - Metal: {}, Energy: {}",
            bot.inventory.get(Item::Metal),
            bot.energy.0
        );

        if bot.energy.0 < 10 && !bot.subsystems.has(Subsystem::Generator) {
            if let Some(bot) = self
                .find_bot(bot, 1, |b| b.subsystems.has(Subsystem::Generator))
            {
                let dir = bot.pos.dir_to(&bot.pos).unwrap();
                return Act(Action::Recharge(dir), "Recharging");
            }
        }

        self.decide_base_map_sharing(state, update)?;

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

        info!(self, "Current tractor count: {}", tractor_count);

        if flea_count == 0 {
            if bot.inventory.get(Item::Metal) >= FrameKind::Flea.build_cost() {
                info!(
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

        info!(self, "Current generator count: {}", generator_count);

        if generator_count <= 1 {
            if bot.inventory.get(Item::Metal)
                >= FrameKind::Building(BuildingKind::Small).build_cost()
            {
                info!(
                    self,
                    "Base has enough metal ({}), building a small building \
                     with generator",
                    bot.inventory.get(Item::Metal)
                );
                let Some(dir) =
                    bot.known_map.find_adj(bot.pos, |cell| !cell.is_blocked())
                else {
                    info!(self, "No valid direction to build generator");
                    return Wait;
                };

                return Act(
                    Action::Build(
                        dir,
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
            info!(
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

        info!(
            self,
            "Base waiting for resources, current metal: {}",
            bot.inventory.get(Item::Metal)
        );
        Wait
    }

    fn decide_base_map_sharing(
        &mut self,
        state: &mut BaseState,
        update: &BotUpdate,
    ) -> DecisionResult {
        let bot = &update.bot_data;
        let current_tick = update.tick;
        let recently_seen_bots = bot.known_bots.iter().filter(|b| {
            current_tick - b.last_observed < 1
                && b.bot_id != self.ctx.bot_id
                && Some(b.bot_id) != bot.known_map.last_received_map_from
        });

        let eligible_for_share = recently_seen_bots.filter(|b| {
            state
                .last_shared_map
                .get(&b.bot_id)
                // If the bot has not been shared with recently
                // it is eligible
                .map(|tick| current_tick - tick > 30)
                // If the bot has not been shared with before, it is eligible
                .unwrap_or(true)
        });

        let Some(bot_to_share_with) =
            eligible_for_share.min_by_key(|b| b.last_observed)
        else {
            return Continue;
        };

        info!(
            self,
            "Sharing map with bot {:?}, last shared map tick: {:?}",
            bot_to_share_with.bot_id,
            state.last_shared_map.get(&bot_to_share_with.bot_id)
        );

        // Update last shared map
        state
            .last_shared_map
            .insert(bot_to_share_with.bot_id, current_tick);

        return Act(
            Action::ShareMap {
                with: bot_to_share_with.bot_id,
            },
            "Sharing map",
        );
    }

    fn init_gatherer_state(update: &BotUpdate) -> GathererState {
        let bot = &update.bot_data;
        let target = Self::find_base(bot).expect("No base found");
        GathererState {
            last_shared_map_tick: update.tick,
            base: target,
        }
    }

    fn find_base(bot: &BotData) -> Option<Pos> {
        bot.known_map
            .find_nearby(bot.pos, 1000, |cell| {
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
            .map(|(pos, _)| pos)
    }

    fn gatherer_behavior(
        &mut self,
        state: &mut GathererState,
        update: &BotUpdate,
    ) -> DecisionResult {
        let bot = &update.bot_data;

        self.ensure_energy(bot)?;

        self.decide_pawn_map_sharing(
            &mut state.last_shared_map_tick,
            &state.base,
            update,
        )?;

        self.wait_for_in_progress_action(&update.in_progress_action)?;

        self.gather(bot, &state.base)?;

        self.explore(bot, 1000)
    }

    fn gather(&mut self, bot: &BotData, base: &Pos) -> DecisionResult {
        // next to base and has metal => Transfer
        if bot.inventory.has(Item::Metal) && bot.pos.is_adjacent(base) {
            let dir = bot.pos.dir_to(base).unwrap();
            info!(
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
            info!(
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
            info!(self, "Found metal at {:?}", target);
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
            info!(
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

    fn init_explorer_state(update: &BotUpdate) -> ExplorerState {
        let base = Self::find_base(&update.bot_data).expect("No base found");
        ExplorerState {
            last_shared_map_tick: update.tick,
            base,
        }
    }

    fn explorer_behavior(
        &mut self,
        state: &mut ExplorerState,
        update: &BotUpdate,
    ) -> DecisionResult {
        let bot = &update.bot_data;
        self.ensure_energy(bot)?;
        self.wait_for_in_progress_action(&update.in_progress_action)?;
        self.explore(bot, 1000)?;

        // Return to base
        let distance = bot.pos.manhattan_distance(&state.base);
        if distance > 10 {
            info!(self, "Explorer has nothing to do, returning to base ",);
            if let Some(path) = bot.known_map.find_path_adj(bot.pos, state.base)
            {
                return Act(
                    Action::MoveTo(path),
                    "Nothing to explore, returning to base",
                );
            }
        }
        Wait
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
            info!(self, "Found unknown cell at {:?}", target);
            if let Some(path) = bot.known_map.find_path(bot.pos, target) {
                return Act(Action::MoveTo(path), "Exploring");
            }
        } else {
            info!(self, "No unknown cells found within range");
        }

        Continue
    }

    fn ensure_energy(&mut self, bot: &BotData) -> DecisionResult {
        let base_pos = match self
            .find_bot(bot, 1000, |b| b.subsystems.has(Subsystem::Generator))
        {
            Some(base) => base.pos,
            None => {
                let Some(base) = self.find_bot(bot, 1000, |b| {
                    b.frame == FrameKind::Building(BuildingKind::Small)
                }) else {
                    info!(self, "No recharge base found, continuing");
                    return Continue;
                };
                base.pos
            }
        };

        let energy_level = bot.energy.0;
        let max_energy = bot.max_energy().0;
        let distance_to_base = bot.pos.manhattan_distance(&base_pos);

        if energy_level < max_energy {
            if let Some(dir) = bot.pos.dir_to(&base_pos) {
                info!(
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
                info!(
                    self,
                    "Low energy: {}/{}, distance to base: {}, moving to base",
                    energy_level,
                    max_energy,
                    distance_to_base
                );
                return self.move_and_act(
                    bot,
                    base_pos,
                    Action::Recharge,
                    "Returning to base to recharge",
                );
            }
        }

        Continue
    }

    fn decide_pawn_map_sharing(
        &mut self,
        last_shared_map_tick: &mut u32,
        base_pos: &Pos,
        update: &BotUpdate,
    ) -> DecisionResult {
        // If map shared recently, don't share
        if update.tick - *last_shared_map_tick < 30 {
            return Continue;
        }

        // If the bot is not in range of base, don't share
        if update.bot_data.pos.manhattan_distance(base_pos) > 5 {
            return Continue;
        }

        let base_cell = update.bot_data.known_map.get(*base_pos);
        let base_id = base_cell.pawn.expect("Base cell has no pawn");

        // Update last shared map
        *last_shared_map_tick = update.tick;

        Act(Action::ShareMap { with: base_id }, "Sharing map with base")
    }

    fn move_and_act(
        &mut self,
        bot: &BotData,
        target: Pos,
        action: impl FnOnce(Dir) -> Action,
        reason: &'static str,
    ) -> DecisionResult {
        if let Some(dir) = bot.pos.dir_to(&target) {
            info!(
                self,
                "Adjacent to target {:?}, taking action in direction: {:?}",
                target,
                dir
            );
            return Act(action(dir), reason);
        }
        if let Some(path) = bot.known_map.find_path_adj(bot.pos, target) {
            info!(
                self,
                "Moving to target {:?}, path length: {}",
                target,
                path.len()
            );
            return Act(Action::MoveTo(path), reason);
        }

        info!(self, "Cannot find path to target {:?}", target);
        Continue
    }

    fn wait_for_in_progress_action(
        &mut self,
        in_progress_action: &Option<ActionWithId>,
    ) -> DecisionResult {
        let Some(in_progress_action) = &in_progress_action else {
            return Continue;
        };
        info!(
            self,
            "Waiting for in_progress_action to complete: {:?} {:?}",
            in_progress_action.id,
            in_progress_action.action.discriminant()
        );
        Wait
    }

    fn find_bot<'a>(
        &'a self,
        bot: &'a BotData,
        max_distance: usize,
        pred: impl Fn(&'a ClientBotData) -> bool,
    ) -> Option<&'a ClientBotData> {
        let mut found = None;
        bot.known_map.find_nearby(bot.pos, max_distance, |cell| {
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
        });
        found
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
        ctx.info(format!(
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
