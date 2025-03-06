use std::io::stdin;

use eyre::Result;
use swarm_lib::{
    bevy_math::UVec2,
    bot_harness::OldBot,
    ctx::Ctx,
    Action,
    BotResponse,
    Dir,
    Pos,
};

pub struct TerminalControlledBot {
    rpc: Ctx,
}

impl OldBot for TerminalControlledBot {
    fn new(rpc: Ctx) -> Self {
        Self { rpc }
    }

    fn run(&mut self) -> Result<()> {
        // Subscribe to position and radar initially
        let initial_response = BotResponse::builder().build();
        self.rpc.send_msg(initial_response);
        self.rpc.info("Terminal-controlled bot initialized");

        loop {
            // Wait for server update
            let Some(update) = self.rpc.wait_for_latest_update() else {
                // Bot was killed
                return Ok(());
            };
            self.rpc
                .info(format!("Received update: tick={}", update.tick));

            // Display radar data visually
            self.rpc.print_radar(&update);

            // Display position and radar data
            self.rpc
                .info(format!("Current position: {:?}", update.position));
            self.rpc.debug(format!(
                "Radar shows {} bots",
                update.radar.pawns.len()
            ));

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

                    // Parse the input as "x y" format
                    let coords: Vec<&str> =
                        coord_input.trim().split_whitespace().collect();

                    // Default to 0,0 if parsing fails
                    let x = if coords.len() > 0 {
                        coords[0].parse::<usize>().unwrap_or(0)
                    } else {
                        0
                    };
                    let y = if coords.len() > 1 {
                        coords[1].parse::<usize>().unwrap_or(0)
                    } else {
                        0
                    };

                    self.rpc
                        .info(format!("Moving to coordinates ({}, {})", x, y));
                    resp.push_action(Action::MoveTo(Pos::from((x, y))));
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
