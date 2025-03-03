#![allow(unused_imports)]

use eyre::Result;
use random_walk::RandomWalkBot;
use swarm_lib::{
    bot_harness::{Bot, Ctx, Harness},
    BotResponse,
    ServerUpdate,
    SubscriptionType,
};

mod crumb;
mod manual;
mod random_walk;

fn main() -> Result<()> {
    let mut harness = Harness::new();
    // harness.register::<RandomWalkBot>("FindBot");
    // harness.register::<TerminalControlledBot>("Basic");
    harness.register::<crumb::CrumbFollower>("Basic");

    harness.run_bots()
}

pub trait CtxExt {
    fn log_debug_info(&mut self, update: &ServerUpdate, log_every_x_ticks: u32);
}

impl CtxExt for Ctx {
    fn log_debug_info(
        &mut self,
        update: &ServerUpdate,
        log_every_x_ticks: u32,
    ) {
        self.debug(format!("Processing tick {}", update.tick));

        if update.tick % log_every_x_ticks == 0 {
            if let Some(pos) = &update.position {
                self.debug(format!("Current position: {:?}", pos));
            }

            if let Some(radar) = &update.radar {
                // Use structured logging for bot detection
                let mut attrs = std::collections::HashMap::new();
                attrs.insert(
                    "num_bots".to_string(),
                    radar.bots.len().to_string(),
                );
                self.log_with_attrs("Radar scan complete", attrs);

                // The print_radar method now logs internally
                self.print_radar(&update);
            }
        }
    }
}

pub trait BotUpdate {
    fn update(&mut self, update: ServerUpdate) -> Option<BotResponse>;
    fn ctx(&mut self) -> &mut Ctx;
}

pub fn run_loop(updater: &mut impl BotUpdate) -> Result<()> {
    // Subscribe to position and radar initially
    let initial_response = BotResponse::builder()
        .subscribe(SubscriptionType::Position)
        .subscribe(SubscriptionType::Radar)
        .build();
    updater.ctx().send_msg(initial_response);
    updater
        .ctx()
        .info("Bot initialized and subscribed to position and radar");

    loop {
        // Wait for server update
        let update = updater.ctx().wait_for_latest_update();
        if update.position.is_none() {
            updater.ctx().warn("Update does not contain position");
            continue;
        }
        if update.radar.is_none() {
            updater.ctx().warn("Update does not contain radar");
            continue;
        }

        // Log debug info every tick
        updater.ctx().log_debug_info(&update, 1);

        if let Some(response) = updater.update(update) {
            updater.ctx().send_msg(response);
        }
    }
}
