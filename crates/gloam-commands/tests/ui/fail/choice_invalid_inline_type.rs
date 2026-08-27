#![allow(unused_imports)]

use gloam_commands::{Context, Result, command};

#[command(description = "Invalid inline choice kind")]
async fn invalid(
    _ctx: Context<()>,
    #[description = "Boolean flag"]
    #[choice(name = "Enabled", value = true)]
    enabled: bool,
) -> Result<()> {
    let _ = enabled;
    Ok(())
}

fn main() {}
