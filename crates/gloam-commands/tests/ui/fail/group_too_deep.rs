#![allow(unused_imports)]

use gloam_commands::{Context, Result, command, group};

struct State;

#[group(description = "Administration commands")]
mod admin {
    use super::{Context, Result, State, command, group};

    #[group(description = "Configuration commands")]
    mod config {
        use super::{Context, Result, State, command, group};

        #[group(description = "Advanced configuration")]
        mod advanced {
            use super::{Context, Result, State, command};

            #[command(description = "Set a value")]
            async fn set(_ctx: Context<State>) -> Result<()> {
                Ok(())
            }
        }
    }
}

fn main() {}
