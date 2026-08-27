#![allow(unused_imports)]

use gloam_commands::{AutocompleteContext, Result, autocomplete};

struct State;

#[autocomplete]
async fn complete(_ctx: AutocompleteContext<State>) -> Result<()> {
    Ok(())
}

fn main() {}
