#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Invalid numeric constraint")]
async fn invalid(
    _ctx: Context<State>,
    #[description = "Text value"]
    #[min = 1]
    value: String,
) -> Result<()> {
    let _ = value;
    Ok(())
}

fn main() {}
