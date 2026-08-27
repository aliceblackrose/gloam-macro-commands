#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Invalid context", context = "thread")]
async fn secure(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

fn main() {}
