use std::{
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use gloam_commands::{
    CheckDescriptor, CheckFuture, CommandDescriptor, CommandPolicy, CommandTask, Context,
    DispatchOutcome, Error, Framework, Result, SlashCommand,
};
use gloamwire::{
    RestClient,
    gateway::{DispatchEvent, GatewayEvent},
    model::{InteractionContextType, Permissions},
};
use tokio::sync::Semaphore;

static POLICY: CommandDescriptor = CommandDescriptor::new("policy", "Exercise execution policy");
static SLOW: CommandDescriptor = CommandDescriptor::new("slow", "Exercise command concurrency");

struct State {
    calls: AtomicUsize,
    order: Mutex<Vec<&'static str>>,
}

impl State {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            order: Mutex::new(Vec::new()),
        }
    }
}

fn handler(ctx: Context<State>) -> gloam_commands::CommandFuture {
    Box::pin(async move {
        ctx.data().calls.fetch_add(1, Ordering::SeqCst);
        ctx.data().order.lock().expect("order mutex").push("handler");
        Ok(())
    })
}

fn first_check(ctx: Context<State>) -> CheckFuture {
    Box::pin(async move {
        ctx.data().order.lock().expect("order mutex").push("first");
        Ok(true)
    })
}

fn second_check(ctx: Context<State>) -> CheckFuture {
    Box::pin(async move {
        ctx.data().order.lock().expect("order mutex").push("second");
        Ok(true)
    })
}

fn denying_check(ctx: Context<State>) -> CheckFuture {
    Box::pin(async move {
        ctx.data().order.lock().expect("order mutex").push("deny");
        Ok(false)
    })
}

#[tokio::test]
async fn evaluates_custom_checks_in_declaration_order_before_handler() -> Result<()> {
    let policy = CommandPolicy::new()
        .check(CheckDescriptor::new("first_check", first_check))
        .check(CheckDescriptor::new("second_check", second_check));
    let framework = Framework::builder(State::new())
        .command(SlashCommand::new_with_policy(&POLICY, handler, policy))
        .build()?;
    let rest = RestClient::new("test-token")?;

    spawned(framework.dispatch(&rest, &dm_event("policy", 10))?)
        .join()
        .await?;

    assert_eq!(
        framework.data().order.lock().expect("order mutex").as_slice(),
        ["first", "second", "handler"]
    );
    assert_eq!(framework.data().calls.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn denies_custom_checks_without_running_handler() -> Result<()> {
    let policy = CommandPolicy::new().check(CheckDescriptor::new("denying_check", denying_check));
    let framework = Framework::builder(State::new())
        .command(SlashCommand::new_with_policy(&POLICY, handler, policy))
        .build()?;
    let rest = RestClient::new("test-token")?;

    let error = spawned(framework.dispatch(&rest, &dm_event("policy", 10))?)
        .join()
        .await
        .expect_err("check should deny command");

    assert!(matches!(
        error,
        Error::CommandCheckFailed { path, check }
            if path == "policy" && check == "denying_check"
    ));
    assert_eq!(framework.data().calls.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn enforces_guild_only_contexts() -> Result<()> {
    let framework = Framework::builder(State::new())
        .command(SlashCommand::new_with_policy(
            &POLICY,
            handler,
            CommandPolicy::new().guild_only(),
        ))
        .build()?;
    let rest = RestClient::new("test-token")?;

    let error = spawned(framework.dispatch(&rest, &dm_event("policy", 10))?)
        .join()
        .await
        .expect_err("DM invocation should be denied");
    assert!(matches!(
        error,
        Error::CommandContextNotAllowed(path) if path == "policy"
    ));

    spawned(framework.dispatch(
        &rest,
        &guild_event(
            "policy",
            10,
            Permissions::empty(),
            Permissions::empty(),
        ),
    )?)
    .join()
    .await?;
    assert_eq!(framework.data().calls.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn enforces_member_and_application_permissions() -> Result<()> {
    let policy = CommandPolicy::new()
        .guild_only()
        .member_permissions(Permissions::BAN_MEMBERS)
        .bot_permissions(Permissions::MANAGE_GUILD);
    let framework = Framework::builder(State::new())
        .command(SlashCommand::new_with_policy(&POLICY, handler, policy))
        .build()?;
    let rest = RestClient::new("test-token")?;

    let member_error = spawned(framework.dispatch(
        &rest,
        &guild_event(
            "policy",
            10,
            Permissions::empty(),
            Permissions::MANAGE_GUILD,
        ),
    )?)
    .join()
    .await
    .expect_err("member permission should be required");
    assert!(matches!(member_error, Error::MissingMemberPermissions { .. }));

    let bot_error = spawned(framework.dispatch(
        &rest,
        &guild_event(
            "policy",
            10,
            Permissions::BAN_MEMBERS,
            Permissions::empty(),
        ),
    )?)
    .join()
    .await
    .expect_err("application permission should be required");
    assert!(matches!(bot_error, Error::MissingBotPermissions { .. }));

    spawned(framework.dispatch(
        &rest,
        &guild_event(
            "policy",
            10,
            Permissions::BAN_MEMBERS,
            Permissions::MANAGE_GUILD,
        ),
    )?)
    .join()
    .await?;
    assert_eq!(framework.data().calls.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn cooldown_is_scoped_per_invoking_user() -> Result<()> {
    let policy = CommandPolicy::new().cooldown(Duration::from_secs(60));
    let framework = Framework::builder(State::new())
        .command(SlashCommand::new_with_policy(&POLICY, handler, policy))
        .build()?;
    let rest = RestClient::new("test-token")?;

    spawned(framework.dispatch(&rest, &dm_event("policy", 10))?)
        .join()
        .await?;

    let cooldown = spawned(framework.dispatch(&rest, &dm_event("policy", 10))?)
        .join()
        .await
        .expect_err("same user should be on cooldown");
    assert!(matches!(
        cooldown,
        Error::CommandOnCooldown { path, retry_after }
            if path == "policy" && !retry_after.is_zero()
    ));

    spawned(framework.dispatch(&rest, &dm_event("policy", 11))?)
        .join()
        .await?;
    assert_eq!(framework.data().calls.load(Ordering::SeqCst), 2);
    Ok(())
}

struct ConcurrencyState {
    entered: AtomicUsize,
    release: Semaphore,
}

impl ConcurrencyState {
    fn new() -> Self {
        Self {
            entered: AtomicUsize::new(0),
            release: Semaphore::new(0),
        }
    }
}

fn slow_handler(ctx: Context<ConcurrencyState>) -> gloam_commands::CommandFuture {
    Box::pin(async move {
        ctx.data().entered.fetch_add(1, Ordering::SeqCst);
        let permit = ctx
            .data()
            .release
            .acquire()
            .await
            .expect("release semaphore remains open");
        permit.forget();
        Ok(())
    })
}

#[tokio::test]
async fn per_command_concurrency_refuses_waiters_before_spawn() -> Result<()> {
    let framework = Framework::builder(ConcurrencyState::new())
        .command(SlashCommand::new_with_policy(
            &SLOW,
            slow_handler,
            CommandPolicy::new().max_concurrency(1),
        ))
        .max_concurrent_commands(2)
        .build()?;
    let rest = RestClient::new("test-token")?;

    let first = spawned(framework.dispatch(&rest, &dm_event("slow", 10))?);
    assert!(matches!(
        framework.dispatch(&rest, &dm_event("slow", 11))?,
        DispatchOutcome::AtCapacity { name } if name == "slow"
    ));

    while framework.data().entered.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    framework.data().release.add_permits(1);
    first.join().await?;

    let second = spawned(framework.dispatch(&rest, &dm_event("slow", 11))?);
    while framework.data().entered.load(Ordering::SeqCst) < 2 {
        tokio::task::yield_now().await;
    }
    framework.data().release.add_permits(1);
    second.join().await?;
    Ok(())
}

fn dm_event(command_name: &str, user_id: u64) -> GatewayEvent {
    interaction_event(
        command_name,
        serde_json::json!({
            "context": InteractionContextType::BOT_DM.0,
            "user": {
                "id": user_id.to_string(),
                "username": "user",
                "discriminator": "0"
            }
        }),
    )
}

fn guild_event(
    command_name: &str,
    user_id: u64,
    member_permissions: Permissions,
    app_permissions: Permissions,
) -> GatewayEvent {
    interaction_event(
        command_name,
        serde_json::json!({
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
        }),
    )
}

fn interaction_event(command_name: &str, extra: serde_json::Value) -> GatewayEvent {
    let mut data = serde_json::json!({
        "id": "100",
        "application_id": "200",
        "type": 2,
        "data": {
            "id": "300",
            "name": command_name,
            "type": 1
        },
        "token": "interaction-token",
        "version": 1
    });
    let object = data.as_object_mut().expect("interaction object");
    object.extend(extra.as_object().expect("extra interaction fields").clone());

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
