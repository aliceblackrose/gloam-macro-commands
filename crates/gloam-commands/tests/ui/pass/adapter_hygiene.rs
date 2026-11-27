use gloam_commands::{
    AutocompleteChoice, AutocompleteContext, Context, Framework, Result, autocomplete, check,
    command, commands,
};

#[check]
async fn ctx(_context: Context<()>) -> Result<bool> {
    Ok(true)
}

#[autocomplete]
async fn __gloam_context(_context: AutocompleteContext<()>) -> Result<Vec<AutocompleteChoice>> {
    Ok(Vec::new())
}

#[command(description = "Check adapter name collisions", check = ctx)]
async fn collide(
    _context: Context<()>,
    #[description = "An option called ctx"]
    #[autocomplete = __gloam_context]
    ctx: String,
    #[description = "An option sharing the handler name"]
    collide: String,
) -> Result<()> {
    let _ = (ctx, collide);
    Ok(())
}

mod command_name {
    use super::*;

    #[command(description = "Handler named ctx")]
    pub async fn ctx(_context: Context<()>) -> Result<()> {
        Ok(())
    }
}

fn main() {
    let _ = Framework::builder(())
        .commands(commands![collide, command_name::ctx])
        .build()
        .expect("valid command registry");
}
