#![allow(unused_imports)]

use gloam_commands::{Result, command};

#[command(description = "Check bot responsiveness")]
async fn ping(value: String) -> Result<()> {
    let _ = value;
    Ok(())
}

fn main() {}
