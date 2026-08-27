use gloam_commands::{Context, Result, check, command, commands};
use gloamwire::model::Permissions;

struct State;

#[check]
async fn allowed(_ctx: Context<State>) -> Result<bool> {
    Ok(true)
}

#[command(
    description = "Restricted command",
    check = allowed,
    context = "guild",
    member_permissions = Permissions::BAN_MEMBERS | Permissions::KICK_MEMBERS,
    bot_permissions = Permissions::MANAGE_GUILD,
    cooldown = 5,
    max_concurrency = 2
)]
async fn secure(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

#[command(description = "Guild-only command", guild_only)]
async fn guild(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

fn main() {
    let commands = commands![secure, guild];
    assert_eq!(commands.len(), 2);

    let policy = commands[1].policy().expect("leaf policy");
    assert_eq!(policy.allowed_contexts().len(), 1);

    let policy = commands[0].policy().expect("leaf policy");
    assert_eq!(policy.checks().len(), 1);
    assert_eq!(policy.cooldown_duration().unwrap().as_secs(), 5);
    assert_eq!(policy.max_concurrent_executions(), Some(2));
    assert_eq!(
        policy.required_member_permissions(),
        Some(Permissions::BAN_MEMBERS | Permissions::KICK_MEMBERS)
    );
    assert_eq!(
        policy.required_bot_permissions(),
        Some(Permissions::MANAGE_GUILD)
    );
}
