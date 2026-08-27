#![allow(unused_imports)]

use gloam_commands::{Context, Result, command, group};

struct State;

#[group]
mod admin {
    use super::{Context, Result, State, command};

    #[command(description = "Ban a member")]
    async fn ban(_ctx: Context<State>) -> Result<()> {
        Ok(())
    }
}

fn main() {}
