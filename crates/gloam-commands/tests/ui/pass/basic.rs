use gloam_commands::{Context, Framework, Result, command, commands};

struct State;

#[command(name = "ping", description = "Check bot responsiveness")]
async fn ping(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

fn main() {
    let framework = Framework::builder(State)
        .commands(commands![ping])
        .build()
        .expect("valid command registry");

    assert_eq!(framework.registry().len(), 1);
    assert_eq!(
        framework
            .registry()
            .get("ping")
            .expect("registered command")
            .descriptor()
            .description,
        "Check bot responsiveness"
    );

    let _original_function = ping;
}
