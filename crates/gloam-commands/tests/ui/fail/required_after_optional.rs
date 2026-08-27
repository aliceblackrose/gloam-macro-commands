#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Invalid option ordering")]
async fn ordering(
    _ctx: Context<State>,
    #[description = "Optional value"] optional: Option<String>,
    #[description = "Required value"] required: String,
) -> Result<()> {
    let _ = (optional, required);
    Ok(())
}

fn main() {}
