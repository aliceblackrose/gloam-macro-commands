#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Echo text")]
async fn echo(_ctx: Context<State>, value: String) -> Result<()> {
    let _ = value;
    Ok(())
}

fn main() {}
