use gloam_commands::{Context, Result, command};

struct State;

#[command(name = "bad name", description = "Invalid command name")]
async fn bad_name(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

fn main() {}
