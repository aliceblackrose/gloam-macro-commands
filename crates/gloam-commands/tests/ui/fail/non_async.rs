use gloam_commands::{Context, Result, command};

struct State;

#[command(description = "Check bot responsiveness")]
fn ping(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

fn main() {}
