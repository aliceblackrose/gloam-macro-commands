use std::sync::Mutex;

use gloam_commands::{
    CommandDescriptor, CommandFuture, CommandTask, Context, DispatchOutcome, Error, Framework,
    Result, SlashCommand,
};
use gloamwire::{RestClient, gateway::{DispatchEvent, GatewayEvent}};

static PING: CommandDescriptor = CommandDescriptor::new("ping", "Check lifecycle behavior");

#[derive(Default)]
struct State {
    events: Mutex<Vec<&'static str>>,
    error: Mutex<Option<String>>,
}

impl State {
    fn record(&self, event: &'static str) {
        self.events.lock().expect("events mutex").push(event);
    }

    fn events(&self) -> Vec<&'static str> {
        self.events.lock().expect("events mutex").clone()
    }
}

fn before_one(ctx: Context<State>) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("before-one");
        Ok(())
    })
}

fn before_two(ctx: Context<State>) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("before-two");
        Ok(())
    })
}

fn before_fail(ctx: Context<State>) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("before-fail");
        Err(Error::MissingOption("before"))
    })
}

fn handler(ctx: Context<State>) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("handler");
        Ok(())
    })
}

fn failing_handler(ctx: Context<State>) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("handler");
        Err(Error::MissingOption("handler"))
    })
}

fn after_one(ctx: Context<State>) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("after-one");
        Ok(())
    })
}

fn after_two(ctx: Context<State>) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("after-two");
        Ok(())
    })
}

fn after_fail_one(ctx: Context<State>) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("after-fail-one");
        Err(Error::MissingOption("after-one"))
    })
}

fn after_fail_two(ctx: Context<State>) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("after-fail-two");
        Err(Error::MissingOption("after-two"))
    })
}

fn handle_error(ctx: Context<State>, error: Error) -> CommandFuture {
    Box::pin(async move {
        ctx.data().record("error");
        *ctx.data().error.lock().expect("error mutex") = Some(error.to_string());
        Ok(())
    })
}

#[tokio::test]
async fn runs_before_handler_and_after_hooks_in_order() -> Result<()> {
    let framework = Framework::builder(State::default())
        .command(SlashCommand::new(&PING, handler))
        .before_command(before_one)
        .before_command(before_two)
        .after_command(after_one)
        .after_command(after_two)
        .build()?;
    let rest = RestClient::new("test-token")?;

    spawned(framework.dispatch(&rest, &interaction_event())?)
        .join()
        .await?;

    assert_eq!(
        framework.data().events(),
        ["before-one", "before-two", "handler", "after-one", "after-two"]
    );
    Ok(())
}

#[tokio::test]
async fn routes_handler_errors_after_running_after_hooks() -> Result<()> {
    let framework = Framework::builder(State::default())
        .command(SlashCommand::new(&PING, failing_handler))
        .before_command(before_one)
        .after_command(after_one)
        .after_command(after_two)
        .command_error_handler(handle_error)
        .build()?;
    let rest = RestClient::new("test-token")?;

    spawned(framework.dispatch(&rest, &interaction_event())?)
        .join()
        .await?;

    assert_eq!(
        framework.data().events(),
        ["before-one", "handler", "after-one", "after-two", "error"]
    );
    assert_eq!(
        framework
            .data()
            .error
            .lock()
            .expect("error mutex")
            .as_deref(),
        Some("missing required slash-command option `handler`")
    );
    Ok(())
}

#[tokio::test]
async fn before_hook_failure_short_circuits_command_and_after_hooks() -> Result<()> {
    let framework = Framework::builder(State::default())
        .command(SlashCommand::new(&PING, handler))
        .before_command(before_fail)
        .before_command(before_two)
        .after_command(after_one)
        .command_error_handler(handle_error)
        .build()?;
    let rest = RestClient::new("test-token")?;

    spawned(framework.dispatch(&rest, &interaction_event())?)
        .join()
        .await?;

    assert_eq!(framework.data().events(), ["before-fail", "error"]);
    Ok(())
}

#[tokio::test]
async fn preserves_first_after_hook_error_while_running_remaining_hooks() -> Result<()> {
    let framework = Framework::builder(State::default())
        .command(SlashCommand::new(&PING, handler))
        .after_command(after_fail_one)
        .after_command(after_fail_two)
        .command_error_handler(handle_error)
        .build()?;
    let rest = RestClient::new("test-token")?;

    spawned(framework.dispatch(&rest, &interaction_event())?)
        .join()
        .await?;

    assert_eq!(
        framework.data().events(),
        ["handler", "after-fail-one", "after-fail-two", "error"]
    );
    assert_eq!(
        framework
            .data()
            .error
            .lock()
            .expect("error mutex")
            .as_deref(),
        Some("missing required slash-command option `after-one`")
    );
    Ok(())
}

#[tokio::test]
async fn propagates_execution_error_without_central_handler() -> Result<()> {
    let framework = Framework::builder(State::default())
        .command(SlashCommand::new(&PING, failing_handler))
        .build()?;
    let rest = RestClient::new("test-token")?;

    let error = spawned(framework.dispatch(&rest, &interaction_event())?)
        .join()
        .await
        .expect_err("handler error should propagate");

    assert!(matches!(error, Error::MissingOption("handler")));
    Ok(())
}

fn interaction_event() -> GatewayEvent {
    GatewayEvent::Dispatch(DispatchEvent {
        name: "INTERACTION_CREATE".to_owned(),
        sequence: 1,
        data: serde_json::json!({
            "id": "100",
            "application_id": "200",
            "type": 2,
            "data": {
                "id": "300",
                "name": "ping",
                "type": 1
            },
            "token": "interaction-token",
            "version": 1
        }),
    })
}

fn spawned(outcome: DispatchOutcome) -> CommandTask {
    match outcome {
        DispatchOutcome::Spawned(task) => task,
        other => panic!("expected spawned command, got {other:?}"),
    }
}
