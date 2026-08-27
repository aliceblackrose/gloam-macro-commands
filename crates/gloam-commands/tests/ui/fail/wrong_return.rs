#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Check bot responsiveness")]
async fn ping(_ctx: Context<State>) -> () {
}

fn main() {}
