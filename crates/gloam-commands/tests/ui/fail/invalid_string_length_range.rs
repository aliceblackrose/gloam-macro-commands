#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Invalid string length range")]
async fn invalid(
    _ctx: Context<State>,
    #[description = "Text value"]
    #[min_length = 10]
    #[max_length = 5]
    value: String,
) -> Result<()> {
    let _ = value;
    Ok(())
}

fn main() {}
