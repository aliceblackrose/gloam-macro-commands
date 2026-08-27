#![allow(unused_imports)]

use gloam_commands::{
    AutocompleteChoice, AutocompleteContext, Context, Result, autocomplete, command,
};

struct State;

#[autocomplete]
async fn complete(_ctx: AutocompleteContext<State>) -> Result<Vec<AutocompleteChoice>> {
    Ok(Vec::new())
}

#[command(description = "Search values")]
async fn search(
    _ctx: Context<State>,
    #[description = "Search query"]
    #[autocomplete = complete]
    #[choice(name = "One", value = "one")]
    query: String,
) -> Result<()> {
    let _ = query;
    Ok(())
}

fn main() {}
