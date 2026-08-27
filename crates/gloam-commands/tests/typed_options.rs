use std::sync::Mutex;

use gloam_commands::{Context, DispatchOutcome, Framework, Result, command, commands};
use gloamwire::{
    RestClient,
    gateway::{DispatchEvent, GatewayEvent},
    model::ApplicationCommandNumericValue,
};

type OptionValues = (String, bool, i64, f64, Option<String>);

struct State {
    captured: Mutex<Option<OptionValues>>,
}

#[command(description = "Capture typed slash-command options")]
async fn capture(
    ctx: Context<State>,
    #[description = "Text value"]
    #[min_length = 1]
    #[max_length = 64]
    text: String,
    #[description = "Boolean value"] enabled: bool,
    #[description = "Integer value"]
    #[min = -5]
    #[max = 10]
    count: i64,
    #[description = "Number value"]
    #[min = 0.25]
    #[max = 2.5]
    ratio: f64,
    #[description = "Optional note"] note: Option<String>,
) -> Result<()> {
    *ctx.data().captured.lock().expect("capture mutex") = Some((text, enabled, count, ratio, note));
    Ok(())
}

#[test]
fn generated_descriptor_matches_typed_signature() -> Result<()> {
    let framework = Framework::builder(State {
        captured: Mutex::new(None),
    })
    .commands(commands![capture])
    .build()?;
    let descriptor = framework
        .registry()
        .get("capture")
        .expect("registered capture command")
        .descriptor();

    assert_eq!(descriptor.options.len(), 5);

    let text = descriptor.options[0];
    assert_eq!(text.name, "text");
    assert!(text.required);
    assert_eq!(text.min_length, Some(1));
    assert_eq!(text.max_length, Some(64));

    let count = descriptor.options[2];
    assert_eq!(
        count.min_value,
        Some(ApplicationCommandNumericValue::Integer(-5))
    );
    assert_eq!(
        count.max_value,
        Some(ApplicationCommandNumericValue::Integer(10))
    );

    let ratio = descriptor.options[3];
    assert_eq!(
        ratio.min_value,
        Some(ApplicationCommandNumericValue::Number(0.25))
    );
    assert_eq!(
        ratio.max_value,
        Some(ApplicationCommandNumericValue::Number(2.5))
    );

    assert!(!descriptor.options[4].required);
    Ok(())
}

#[tokio::test]
async fn dispatch_extracts_typed_options_before_invoking_handler() -> Result<()> {
    let framework = Framework::builder(State {
        captured: Mutex::new(None),
    })
    .commands(commands![capture])
    .build()?;
    let rest = RestClient::new("test-token")?;
    let event = GatewayEvent::Dispatch(DispatchEvent {
        name: "INTERACTION_CREATE".to_owned(),
        sequence: 1,
        data: serde_json::json!({
            "id": "100",
            "application_id": "200",
            "type": 2,
            "data": {
                "id": "300",
                "name": "capture",
                "type": 1,
                "options": [
                    {"name": "text", "type": 3, "value": "hello"},
                    {"name": "enabled", "type": 5, "value": true},
                    {"name": "count", "type": 4, "value": 7},
                    {"name": "ratio", "type": 10, "value": 1.5}
                ]
            },
            "token": "interaction-token",
            "version": 1
        }),
    });

    let task = match framework.dispatch(&rest, &event)? {
        DispatchOutcome::Spawned(task) => task,
        other => panic!("expected spawned command, got {other:?}"),
    };
    task.join().await?;

    assert_eq!(
        framework
            .data()
            .captured
            .lock()
            .expect("capture mutex")
            .as_ref(),
        Some(&("hello".to_owned(), true, 7, 1.5, None))
    );
    Ok(())
}
