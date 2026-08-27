use std::sync::atomic::{AtomicUsize, Ordering};

use gloam_commands::{CommandTask, Context, DispatchOutcome, Error, Framework, Result, check, command, commands};
use gloamwire::{
    RestClient,
    gateway::{DispatchEvent, GatewayEvent},
    model::{InteractionContextType, Permissions},
};

struct State {
    checks: AtomicUsize,
    calls: AtomicUsize,
}

impl State {
    const fn new() -> Self {
        Self {
            checks: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
        }
    }
}

#[check]
async fn allowed(ctx: Context<State>) -> Result<bool> {
    ctx.data().checks.fetch_add(1, Ordering::SeqCst);
    Ok(true)
}

#[command(
    description = "Restricted command",
    check = allowed,
    guild_only,
    member_permissions = Permissions::BAN_MEMBERS,
    bot_permissions = Permissions::MANAGE_GUILD,
    cooldown = 60,
    max_concurrency = 1
)]
async fn secure(ctx: Context<State>) -> Result<()> {
    ctx.data().calls.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

#[tokio::test]
async fn generated_policy_is_enforced_during_dispatch() -> Result<()> {
    let framework = Framework::builder(State::new())
        .commands(commands![secure])
        .build()?;
    let rest = RestClient::new("test-token")?;

    let dm_error = spawned(framework.dispatch(&rest, &dm_event(10))?)
        .join()
        .await
        .expect_err("guild-only policy should reject DMs");
    assert!(matches!(dm_error, Error::CommandContextNotAllowed(path) if path == "secure"));
    assert_eq!(framework.data().checks.load(Ordering::SeqCst), 0);

    let permission_error = spawned(framework.dispatch(
        &rest,
        &guild_event(10, Permissions::empty(), Permissions::MANAGE_GUILD),
    )?)
    .join()
    .await
    .expect_err("member permission policy should run before custom checks");
    assert!(matches!(permission_error, Error::MissingMemberPermissions { .. }));
    assert_eq!(framework.data().checks.load(Ordering::SeqCst), 0);

    spawned(framework.dispatch(
        &rest,
        &guild_event(10, Permissions::BAN_MEMBERS, Permissions::MANAGE_GUILD),
    )?)
    .join()
    .await?;
    assert_eq!(framework.data().checks.load(Ordering::SeqCst), 1);
    assert_eq!(framework.data().calls.load(Ordering::SeqCst), 1);

    let cooldown_error = spawned(framework.dispatch(
        &rest,
        &guild_event(10, Permissions::BAN_MEMBERS, Permissions::MANAGE_GUILD),
    )?)
    .join()
    .await
    .expect_err("same user should be on cooldown");
    assert!(matches!(
        cooldown_error,
        Error::CommandOnCooldown { path, retry_after }
            if path == "secure" && !retry_after.is_zero()
    ));
    assert_eq!(framework.data().checks.load(Ordering::SeqCst), 2);
    assert_eq!(framework.data().calls.load(Ordering::SeqCst), 1);
    Ok(())
}

fn dm_event(user_id: u64) -> GatewayEvent {
    interaction_event(serde_json::json!({
        "context": InteractionContextType::BOT_DM.0,
        "user": {
            "id": user_id.to_string(),
            "username": "user",
            "discriminator": "0"
        }
    }))
}

fn guild_event(
    user_id: u64,
    member_permissions: Permissions,
    app_permissions: Permissions,
) -> GatewayEvent {
    interaction_event(serde_json::json!({
        "context": InteractionContextType::GUILD.0,
        "guild_id": "400",
        "member": {
            "user": {
                "id": user_id.to_string(),
                "username": "user",
                "discriminator": "0"
            },
            "roles": [],
            "permissions": member_permissions.bits().to_string()
        },
        "app_permissions": app_permissions.bits().to_string()
    }))
}

fn interaction_event(extra: serde_json::Value) -> GatewayEvent {
    let mut data = serde_json::json!({
        "id": "100",
        "application_id": "200",
        "type": 2,
        "data": {
            "id": "300",
            "name": "secure",
            "type": 1
        },
        "token": "interaction-token",
        "version": 1
    });
    data.as_object_mut()
        .expect("interaction object")
        .extend(extra.as_object().expect("extra interaction fields").clone());

    GatewayEvent::Dispatch(DispatchEvent {
        name: "INTERACTION_CREATE".to_owned(),
        sequence: 1,
        data,
    })
}

fn spawned(outcome: DispatchOutcome) -> CommandTask {
    match outcome {
        DispatchOutcome::Spawned(task) => task,
        other => panic!("expected spawned command, got {other:?}"),
    }
}
