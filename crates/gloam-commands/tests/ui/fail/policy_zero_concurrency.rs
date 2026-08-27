#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Invalid concurrency", max_concurrency = 0)]
async fn secure(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

fn main() {}
