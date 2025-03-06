#![allow(unused_imports)]

use eyre::Result;
use swarm_lib::bot_harness::Harness;

fn main() -> Result<()> {
    let mut harness = Harness::new();
    // harness.register::<RandomWalkBot>("FindBot");
    // harness.register::<TerminalControlledBot>("Basic");
    harness.register::<simple_bot::crumb::CrumbFollower>("Basic");

    harness.run_bots()
}
