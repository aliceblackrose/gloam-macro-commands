#![allow(unused_imports)]

use gloam_commands::{Context, Result, check};

struct State;

#[check]
fn predicate(_ctx: Context<State>) -> Result<bool> {
    Ok(true)
}

fn main() {}
