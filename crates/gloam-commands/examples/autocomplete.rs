use gloam_commands::prelude::*;
use gloamwire::model::ApplicationCommandInteractionValue;

struct State;

#[autocomplete]
async fn complete_query(ctx: AutocompleteContext<State>) -> Result<Vec<AutocompleteChoice>> {
    let partial = match ctx.focused_value() {
        Some(ApplicationCommandInteractionValue::String(value)) => value,
        _ => return Ok(Vec::new()),
    };

    Ok(["alpha", "beta", "gamma"]
        .into_iter()
        .filter(|value| value.starts_with(partial))
        .map(|value| AutocompleteChoice::string(value, value))
        .collect())
}

#[command(description = "Search values")]
async fn search(
    ctx: Context<State>,
    #[description = "Search query"]
    #[autocomplete = complete_query]
    query: String,
) -> Result<()> {
    ctx.reply(format!("Searching for {query}")).await?;
    Ok(())
}

fn main() -> Result<()> {
    let _framework = Framework::builder(State)
        .commands(commands![search])
        .build()?;
    Ok(())
}
