use gloam_commands::prelude::*;

struct State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, CommandChoice)]
enum Mode {
    #[choice(name = "Fast", value = "fast")]
    Fast,
    #[choice(name = "Safe", value = "safe")]
    Safe,
}

#[command(description = "Configure execution")]
async fn configure(
    ctx: Context<State>,
    #[description = "Execution mode"]
    #[choice]
    mode: Mode,
    #[description = "Retry count"]
    #[min = 0]
    #[max = 5]
    retries: i64,
    #[description = "Optional note"] note: Option<String>,
) -> Result<()> {
    ctx.reply(format!(
        "Configured {mode:?} with {retries} retries and note {note:?}"
    ))
    .await?;
    Ok(())
}

fn main() -> Result<()> {
    let _framework = Framework::builder(State)
        .commands(commands![configure])
        .build()?;
    Ok(())
}
