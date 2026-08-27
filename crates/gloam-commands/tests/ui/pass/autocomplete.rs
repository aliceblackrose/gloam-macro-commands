use gloam_commands::{
    AutocompleteChoice, AutocompleteContext, Context, Framework, Result, autocomplete, command,
    commands,
};

struct State;

mod helpers {
    use super::*;

    #[autocomplete]
    pub async fn text(_ctx: AutocompleteContext<State>) -> Result<Vec<AutocompleteChoice>> {
        Ok(vec![AutocompleteChoice::string("Result", "result")])
    }
}

#[autocomplete]
async fn integer(_ctx: AutocompleteContext<State>) -> Result<Vec<AutocompleteChoice>> {
    Ok(vec![AutocompleteChoice::integer("One", 1)])
}

#[autocomplete]
async fn number(_ctx: AutocompleteContext<State>) -> Result<Vec<AutocompleteChoice>> {
    Ok(vec![AutocompleteChoice::number("Half", 0.5)])
}

#[command(description = "Search dynamic values")]
async fn search(
    _ctx: Context<State>,
    #[description = "Search text"]
    #[autocomplete = helpers::text]
    _query: String,
    #[description = "Integer filter"]
    #[autocomplete = integer]
    _count: i64,
    #[description = "Optional numeric score"]
    #[autocomplete = number]
    _score: Option<f64>,
) -> Result<()> {
    Ok(())
}

fn main() {
    let framework = Framework::builder(State)
        .commands(commands![search])
        .build()
        .expect("valid autocomplete command");
    let command = framework.registry().get("search").expect("search command");

    assert!(command.descriptor().options.iter().all(|option| option.autocomplete));
    assert_eq!(command.autocomplete_handlers().len(), 3);
}
