use gloam_commands::{Context, Framework, Result, command, commands, group};

struct State;

#[group(description = "Administration commands")]
mod admin {
    use super::{Context, Result, State, command, group};

    #[command(description = "Ban a member")]
    async fn ban(_ctx: Context<State>) -> Result<()> {
        Ok(())
    }

    #[group(description = "Configuration commands")]
    mod config {
        use super::{Context, Result, State, command};

        #[command(description = "Set a value")]
        async fn set(
            _ctx: Context<State>,
            #[description = "Configured value"] value: i64,
        ) -> Result<()> {
            let _ = value;
            Ok(())
        }
    }
}

fn main() {
    let framework = Framework::builder(State)
        .commands(commands![admin])
        .build()
        .expect("valid group registry");

    let admin = framework.registry().get("admin").expect("admin group");
    assert!(admin.is_group());
    assert_eq!(admin.children().len(), 2);
    assert_eq!(admin.children()[0].descriptor().name, "ban");
    assert_eq!(admin.children()[1].descriptor().name, "config");
    assert!(admin.children()[1].is_group());
    assert_eq!(admin.children()[1].children()[0].descriptor().name, "set");
}
