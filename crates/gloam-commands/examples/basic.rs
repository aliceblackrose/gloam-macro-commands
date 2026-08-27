use gloam_commands::prelude::*;
use gloamwire::model::GuildId;

struct State;

#[command(description = "Say hello")]
async fn hello(
    ctx: Context<State>,
    #[description = "Person to greet"]
    #[min_length = 1]
    #[max_length = 64]
    name: String,
) -> Result<()> {
    ctx.reply(format!("Hello, {name}!" )).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let framework = Framework::builder(State)
        .commands(commands![hello])
        .registration(Registration::Guild(GuildId::new(123456789012345678)))
        .build()?;

    framework
        .run(std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set"))
        .await
}
