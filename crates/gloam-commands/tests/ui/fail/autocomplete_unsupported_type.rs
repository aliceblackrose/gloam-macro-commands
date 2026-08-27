#![allow(unused_imports)]

use gloam_commands::{
    AutocompleteChoice, AutocompleteContext, Context, Result, autocomplete, command,
};

struct State;

#[autocomplete]
async fn complete(_ctx: AutocompleteContext<State>) -> Result<Vec<AutocompleteChoice>> {
    Ok(Vec::new())
}

#[command(description = "Toggle a value")]
async fn toggle(
    _ctx: Context<State>,
    #[description = "Whether enabled"]
    #[autocomplete = complete]
    enabled: bool,
) -> Result<()> {
    let _ = enabled;
    Ok(())
}

fn main() {}
