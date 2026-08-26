use gloam_commands::{Context, Framework, Result, command, commands};

struct State;

mod admin {
    use super::{Context, Result, State, command};

    #[command(description = "Ban a member")]
    pub(crate) async fn ban(_ctx: Context<State>) -> Result<()> {
        Ok(())
    }
}

fn main() {
    let framework = Framework::builder(State)
        .commands(commands![admin::ban])
        .build()
        .expect("valid command registry");

    assert!(framework.registry().get("ban").is_some());
}
