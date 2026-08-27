use gloam_commands::prelude::*;

struct State;

#[group(description = "Administration commands")]
mod admin {
    use super::*;

    #[command(description = "Show service status")]
    async fn status(ctx: Context<State>) -> Result<()> {
        ctx.reply("Service is healthy").await?;
        Ok(())
    }

    #[group(description = "Configuration commands")]
    mod config {
        use super::*;

        #[command(description = "Set a configured value")]
        async fn set(
            ctx: Context<State>,
            #[description = "Configured value"] count: i64,
        ) -> Result<()> {
            debug_assert_eq!(ctx.command_path(), ["admin", "config", "set"]);
            ctx.reply(format!("Configured {count}")).await?;
            Ok(())
        }
    }
}

fn main() -> Result<()> {
    let _framework = Framework::builder(State)
        .commands(commands![admin])
        .build()?;
    Ok(())
}
