use gloam_commands::{Context, Framework, Result, command, commands};
use gloamwire::model::{AttachmentId, ChannelId, RoleId, UserId};

struct State;

#[command(description = "Inspect typed slash-command options")]
async fn inspect(
    _ctx: Context<State>,
    #[description = "Text value"] text: String,
    #[description = "Boolean value"] enabled: bool,
    #[description = "Integer value"] count: i64,
    #[description = "Number value"] ratio: f64,
    #[description = "User value"] user: UserId,
    #[description = "Channel value"] channel: ChannelId,
    #[description = "Role value"] role: RoleId,
    #[description = "Attachment value"] attachment: AttachmentId,
    #[description = "Optional text"] query: Option<String>,
) -> Result<()> {
    let _ = (
        text, enabled, count, ratio, user, channel, role, attachment, query,
    );
    Ok(())
}

fn main() {
    let framework = Framework::builder(State)
        .commands(commands![inspect])
        .build()
        .expect("valid typed command registry");

    let descriptor = framework
        .registry()
        .get("inspect")
        .expect("registered command")
        .descriptor();
    assert_eq!(descriptor.options.len(), 9);
    assert!(descriptor.options[0].required);
    assert!(!descriptor.options[8].required);

    let _original_function = inspect;
}
