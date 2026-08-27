#![allow(unused_imports)]

use gloam_commands::{AutocompleteChoice, Context, Result, autocomplete};

struct State;

#[autocomplete]
async fn complete(_ctx: Context<State>) -> Result<Vec<AutocompleteChoice>> {
    Ok(Vec::new())
}

fn main() {}
