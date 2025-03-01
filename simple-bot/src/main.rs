use std::io::stdin;

use eyre::Result;
use rand::Rng;
use swarm_lib::{
    bevy_math::UVec2,
    bot_harness::{Bot, Harness, Rpc},
    Action,
    BotResponse,
    Dir,
    SubscriptionType,
};

fn main() -> Result<()> {
    let mut harness = Harness::new();
    harness.register::<RandomWalkBot>("FindBot");
    harness.register::<TerminalControlledBot>("Basic");

    harness.run_bots()
}

struct RandomWalkBot {
    rpc: Rpc,
}

impl Bot for RandomWalkBot {
    fn new(rpc: Rpc) -> Self {
        Self { rpc }
    }

    fn run(mut self) -> Result<()> {
        // Subscribe to position and radar initially
        let initial_response = BotResponse::builder()
            .subscribe(SubscriptionType::Position)
            .subscribe(SubscriptionType::Radar)
            .build();
        self.rpc.send_msg(initial_response);
        self.rpc
            .info("Bot initialized and subscribed to position and radar");

        let mut rng = rand::thread_rng();

        loop {
            // Wait for server update
            let update = self.rpc.wait_for_latest_update();

            // Log debug info every tick
            if update.response.tick % 1 == 0 {
                self.rpc
                    .debug(format!("Processing tick {}", update.response.tick));

                if let Some(pos) = &update.response.position {
                    self.rpc.debug(format!("Current position: {:?}", pos));
                }

                if let Some(radar) = &update.response.radar {
                    // Use structured logging for bot detection
                    let mut attrs = std::collections::HashMap::new();
                    attrs.insert(
                        "num_bots".to_string(),
                        radar.bots.len().to_string(),
                    );
                    self.rpc.log_with_attrs("Radar scan complete", attrs);

                    // The print_radar method now logs internally
                    self.rpc.print_radar(&update);
                }
            }

            // Choose a random direction to move
            let direction = match rng.gen_range(0..4) {
                0 => Dir::Up,
                1 => Dir::Down,
                2 => Dir::Left,
                3 => Dir::Right,
                _ => unreachable!(),
            };

            self.rpc.info(format!("Moving {:?}", direction));

            // Build and send response with random movement
            let response = BotResponse::builder()
                .push_action(Action::MoveDir(direction))
                .build();

            self.rpc.send_msg(response);
        }
    }
}

struct TerminalControlledBot {
    rpc: Rpc,
}

impl Bot for TerminalControlledBot {
    fn new(rpc: Rpc) -> Self {
        Self { rpc }
    }

    fn run(mut self) -> Result<()> {
        // Subscribe to position and radar initially
        let initial_response = BotResponse::builder()
            .subscribe(SubscriptionType::Position)
            .subscribe(SubscriptionType::Radar)
            .build();
        self.rpc.send_msg(initial_response);
        self.rpc.info("Terminal-controlled bot initialized");

        loop {
            // Wait for server update
            let update = self.rpc.wait_for_latest_update();
            self.rpc.info(format!(
                "Received update: tick={}",
                update.response.tick
            ));

            // Display radar data visually
            self.rpc.print_radar(&update);

            if let Some(pos) = &update.response.position {
                self.rpc.info(format!("Current position: {:?}", pos));
            }

            if let Some(radar) = &update.response.radar {
                self.rpc
                    .debug(format!("Radar shows {} bots", radar.bots.len()));
            }

            // User interaction prompts still use println since they're for
            // direct user interaction
            println!(
                "Enter command (move-up, move-down, move-left, move-right, \
                 move-to, wait):"
            );

            let mut input = String::new();
            stdin().read_line(&mut input).unwrap();
            let command = input.trim();

            let mut resp = BotResponse::builder();
            match command {
                "move-up" => {
                    self.rpc.info("Moving UP");
                    resp.push_action(Action::MoveDir(Dir::Up));
                }
                "move-down" => {
                    self.rpc.info("Moving DOWN");
                    resp.push_action(Action::MoveDir(Dir::Down));
                }
                "move-left" => {
                    self.rpc.info("Moving LEFT");
                    resp.push_action(Action::MoveDir(Dir::Left));
                }
                "move-right" => {
                    self.rpc.info("Moving RIGHT");
                    resp.push_action(Action::MoveDir(Dir::Right));
                }
                "move-to" => {
                    println!("Enter x y coordinate:");
                    let mut coord_input = String::new();
                    stdin().read_line(&mut coord_input).unwrap();

                    // Let me parse the input as "x y" format
                    let coords: Vec<&str> =
                        coord_input.trim().split_whitespace().collect();

                    // Default to 0,0 if parsing fails
                    let x = if coords.len() > 0 {
                        coords[0].parse::<u32>().unwrap_or(0)
                    } else {
                        0
                    };
                    let y = if coords.len() > 1 {
                        coords[1].parse::<u32>().unwrap_or(0)
                    } else {
                        0
                    };

                    self.rpc
                        .info(format!("Moving to coordinates ({}, {})", x, y));
                    resp.push_action(Action::MoveTo(UVec2::new(x, y)));
                }
                _ => {
                    self.rpc.warn(format!("Unknown command: '{}'", command));
                }
            }

            // Send the response
            self.rpc.send_msg(resp.build());
        }
    }
}
