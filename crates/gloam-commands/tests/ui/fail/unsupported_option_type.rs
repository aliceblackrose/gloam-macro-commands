#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Use unsupported option")]
async fn unsupported(
    _ctx: Context<State>,
    #[description = "Unsupported value"] value: u32,
) -> Result<()> {
    let _ = value;
    Ok(())
}

fn main() {}
