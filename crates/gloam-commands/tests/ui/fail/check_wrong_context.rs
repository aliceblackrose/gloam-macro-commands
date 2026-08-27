#![allow(unused_imports)]

use gloam_commands::{Result, check};

#[check]
async fn predicate(_ctx: ()) -> Result<bool> {
    Ok(true)
}

fn main() {}
