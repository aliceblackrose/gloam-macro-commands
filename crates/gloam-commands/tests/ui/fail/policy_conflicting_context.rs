#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Conflicting context", guild_only, context = "guild")]
async fn secure(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

fn main() {}
