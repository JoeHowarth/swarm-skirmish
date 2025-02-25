use std::io::stdin;

use eyre::Result;
use rand::Rng;
use swarm_lib::{
    bevy_math::UVec2,
    bot_harness::{run_bots, Bot, Rpc},
    Action,
    BotResponse,
    Dir,
    SubscriptionType,
};

fn main() -> Result<()> {
    run_bots::<RandomWalkBot>()
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

        let mut rng = rand::thread_rng();

        loop {
            // Wait for server update
            let update = self.rpc.wait_for_latest_update();

            // Occasionally print debug info
            if update.response.tick % 1 == 0 {
                println!("RandomWalkBot: tick={}", update.response.tick);

                if let Some(pos) = &update.response.position {
                    println!("Current position: {:?}", pos);
                }

                if let Some(radar) = &update.response.radar {
                    println!("Radar shows bots: {:?}", radar.bots);
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
            // let direction = Dir::Right;

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

        loop {
            // Wait for server update
            let update = self.rpc.wait_for_latest_update();
            println!("Received update: tick={}", update.response.tick);

            // if update.response.tick % 4 == 0 {
            self.rpc.print_radar(&update);
            // }

            if let Some(pos) = &update.response.position {
                println!("Current position: {:?}", pos);
            }

            if let Some(radar) = &update.response.radar {
                println!("Radar shows {} bots", radar.bots.len());
            }

            // Read user input for next action
            println!(
                "Enter command (move-forward, move-backward, move-left, \
                 move-right, move-to, wait):"
            );
            let mut input = String::new();
            stdin().read_line(&mut input).unwrap();

            let mut resp = BotResponse::builder();
            match input.trim() {
                "move-up" => {
                    resp.push_action(Action::MoveDir(Dir::Up));
                }
                "move-down" => {
                    resp.push_action(Action::MoveDir(Dir::Down));
                }
                "move-left" => {
                    resp.push_action(Action::MoveDir(Dir::Left));
                }
                "move-right" => {
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

                    resp.push_action(Action::MoveTo(UVec2::new(x, y)));
                }
                _ => {}
            }

            // Send the response
            self.rpc.send_msg(resp.build());
        }
    }
}
