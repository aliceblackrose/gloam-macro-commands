use gloam_commands::{Context, Result, command};

struct State;

#[command(name = "Ping", description = "Check bot responsiveness")]
async fn ping(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

fn main() {}
