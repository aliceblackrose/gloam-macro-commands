use std::sync::Mutex;

use gloam_commands::{
    CommandChoiceValue, Context, DispatchOutcome, Error, Framework, Result, command, commands,
};
use gloamwire::{
    RestClient,
    gateway::{DispatchEvent, GatewayEvent},
    model::ApplicationCommandOptionType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, gloam_commands::CommandChoice)]
enum Mode {
    #[choice(name = "Fast", value = "fast")]
    Fast,
    #[choice(name = "Safe", value = "safe")]
    Safe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, gloam_commands::CommandChoice)]
enum Level {
    #[choice(name = "Low", value = 1)]
    Low,
    #[choice(name = "High", value = 2)]
    High,
}

type Captured = (Mode, String, Option<Level>);

struct State {
    captured: Mutex<Option<Captured>>,
}

#[command(description = "Configure choice-backed options")]
async fn configure(
    ctx: Context<State>,
    #[description = "Execution mode"]
    #[choice]
    mode: Mode,
    #[description = "Output format"]
    #[choice(name = "Text", value = "text")]
    #[choice(name = "JSON", value = "json")]
    format: String,
    #[description = "Optional level"]
    #[choice]
    level: Option<Level>,
) -> Result<()> {
    *ctx.data().captured.lock().expect("capture mutex") = Some((mode, format, level));
    Ok(())
}

#[test]
fn generated_descriptor_contains_typed_and_inline_choices() -> Result<()> {
    let framework = framework()?;
    let descriptor = framework
        .registry()
        .get("configure")
        .expect("registered configure command")
        .descriptor();

    assert_eq!(descriptor.options.len(), 3);

    let mode = descriptor.options[0];
    assert_eq!(mode.kind, ApplicationCommandOptionType::STRING);
    assert!(mode.required);
    assert_eq!(mode.choices.len(), 2);
    assert_eq!(mode.choices[0].name, "Fast");
    assert_eq!(mode.choices[0].value, CommandChoiceValue::String("fast"));
    assert_eq!(mode.choices[1].name, "Safe");
    assert_eq!(mode.choices[1].value, CommandChoiceValue::String("safe"));

    let format = descriptor.options[1];
    assert_eq!(format.kind, ApplicationCommandOptionType::STRING);
    assert!(format.required);
    assert_eq!(format.choices.len(), 2);
    assert_eq!(format.choices[0].value, CommandChoiceValue::String("text"));
    assert_eq!(format.choices[1].value, CommandChoiceValue::String("json"));

    let level = descriptor.options[2];
    assert_eq!(level.kind, ApplicationCommandOptionType::INTEGER);
    assert!(!level.required);
    assert_eq!(level.choices.len(), 2);
    assert_eq!(level.choices[0].value, CommandChoiceValue::Integer(1));
    assert_eq!(level.choices[1].value, CommandChoiceValue::Integer(2));
    Ok(())
}

#[tokio::test]
async fn dispatch_extracts_choice_enums_before_invoking_handler() -> Result<()> {
    let framework = framework()?;
    let rest = RestClient::new("test-token")?;

    let task = spawned(framework.dispatch(&rest, &interaction_event("safe", "json", Some(2)))?);
    task.join().await?;

    assert_eq!(
        framework
            .data()
            .captured
            .lock()
            .expect("capture mutex")
            .as_ref(),
        Some(&(Mode::Safe, "json".to_owned(), Some(Level::High)))
    );
    Ok(())
}

#[tokio::test]
async fn optional_choice_enum_allows_missing_option() -> Result<()> {
    let framework = framework()?;
    let rest = RestClient::new("test-token")?;

    let task = spawned(framework.dispatch(&rest, &interaction_event("fast", "text", None))?);
    task.join().await?;

    assert_eq!(
        framework
            .data()
            .captured
            .lock()
            .expect("capture mutex")
            .as_ref(),
        Some(&(Mode::Fast, "text".to_owned(), None))
    );
    Ok(())
}

#[tokio::test]
async fn rejects_unregistered_typed_choice_value() -> Result<()> {
    let framework = framework()?;
    let rest = RestClient::new("test-token")?;

    let error = spawned(framework.dispatch(&rest, &interaction_event("turbo", "text", None))?)
        .join()
        .await
        .expect_err("unknown typed choice must fail extraction");

    assert!(matches!(error, Error::InvalidChoice { name: "mode" }));
    assert!(
        framework
            .data()
            .captured
            .lock()
            .expect("capture mutex")
            .is_none()
    );
    Ok(())
}

fn framework() -> Result<Framework<State>> {
    Framework::builder(State {
        captured: Mutex::new(None),
    })
    .commands(commands![configure])
    .build()
}

fn interaction_event(mode: &str, format: &str, level: Option<i64>) -> GatewayEvent {
    let mut options = vec![
        serde_json::json!({"name": "mode", "type": 3, "value": mode}),
        serde_json::json!({"name": "format", "type": 3, "value": format}),
    ];
    if let Some(level) = level {
        options.push(serde_json::json!({"name": "level", "type": 4, "value": level}));
    }

    GatewayEvent::Dispatch(DispatchEvent {
        name: "INTERACTION_CREATE".to_owned(),
        sequence: 1,
        data: serde_json::json!({
            "id": "100",
            "application_id": "200",
            "type": 2,
            "data": {
                "id": "300",
                "name": "configure",
                "type": 1,
                "options": options
            },
            "token": "interaction-token",
            "version": 1
        }),
    })
}

fn spawned(outcome: DispatchOutcome) -> gloam_commands::CommandTask {
    match outcome {
        DispatchOutcome::Spawned(task) => task,
        other => panic!("expected spawned command, got {other:?}"),
    }
}
