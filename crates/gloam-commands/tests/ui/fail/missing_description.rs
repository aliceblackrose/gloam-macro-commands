use gloam_commands::{Context, Result, command};

struct State;

#[command]
async fn ping(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

fn main() {}
