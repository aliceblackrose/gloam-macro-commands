use std::sync::Mutex;

use gloam_commands::prelude::*;
use gloamwire::{
    RestClient,
    gateway::{DispatchEvent, GatewayEvent},
    model::ApplicationCommandInteractionValue,
};

struct State {
    path: Mutex<Vec<&'static str>>,
    focused_name: Mutex<String>,
    focused_value: Mutex<String>,
}

impl State {
    fn new() -> Self {
        Self {
            path: Mutex::new(Vec::new()),
            focused_name: Mutex::new(String::new()),
            focused_value: Mutex::new(String::new()),
        }
    }
}

#[group(description = "Administration commands")]
mod admin {
    use super::*;

    #[group(description = "Configuration commands")]
    mod config {
        use super::*;

        #[autocomplete]
        async fn complete_query(
            ctx: AutocompleteContext<State>,
        ) -> Result<Vec<AutocompleteChoice>> {
            let Some(ApplicationCommandInteractionValue::String(value)) = ctx.focused_value()
            else {
                panic!("expected focused string value");
            };

            *ctx.data().path.lock().expect("path mutex") = ctx.command_path().to_vec();
            *ctx.data().focused_name.lock().expect("name mutex") = ctx.focused_name().to_owned();
            *ctx.data().focused_value.lock().expect("value mutex") = value.clone();

            Ok((0..26)
                .map(|index| {
                    AutocompleteChoice::string(format!("Result {index}"), format!("result-{index}"))
                })
                .collect())
        }

        #[command(description = "Search configuration values")]
        async fn search(
            _ctx: Context<State>,
            #[description = "Search query"]
            #[autocomplete = complete_query]
            query: String,
        ) -> Result<()> {
            let _ = query;
            Ok(())
        }
    }
}

#[tokio::test]
async fn macro_autocomplete_routes_nested_focus_before_response_validation() -> Result<()> {
    let framework = Framework::builder(State::new())
        .commands(commands![admin])
        .build()?;
    let rest = RestClient::new("test-token")?;

    let admin = framework.registry().get("admin").expect("admin group");
    let config = &admin.children()[0];
    let search = &config.children()[0];
    assert_eq!(search.descriptor().name, "search");
    assert_eq!(search.descriptor().options.len(), 1);
    assert!(search.descriptor().options[0].autocomplete);
    assert!(search.descriptor().options[0].choices.is_empty());

    let event = autocomplete_event(true);
    let DispatchOutcome::Spawned(task) = framework.dispatch(&rest, &event)? else {
        panic!("expected autocomplete handler to spawn");
    };

    match task.join().await {
        Err(Error::InvalidAutocompleteResponse(message)) => {
            assert!(message.contains("at most 25 choices"));
        }
        other => panic!("expected autocomplete response validation error, got {other:?}"),
    }

    assert_eq!(
        framework.data().path.lock().expect("path mutex").as_slice(),
        ["admin", "config", "search"]
    );
    assert_eq!(
        framework
            .data()
            .focused_name
            .lock()
            .expect("name mutex")
            .as_str(),
        "query"
    );
    assert_eq!(
        framework
            .data()
            .focused_value
            .lock()
            .expect("value mutex")
            .as_str(),
        "par"
    );
    Ok(())
}

#[tokio::test]
async fn autocomplete_requires_exactly_one_focused_leaf_option() -> Result<()> {
    let framework = Framework::builder(State::new())
        .commands(commands![admin])
        .build()?;
    let rest = RestClient::new("test-token")?;

    assert!(matches!(
        framework.dispatch(&rest, &autocomplete_event(false)),
        Err(Error::InvalidAutocompleteFocus(path)) if path == "admin config search"
    ));
    Ok(())
}

fn autocomplete_event(focused: bool) -> GatewayEvent {
    GatewayEvent::Dispatch(DispatchEvent {
        name: "INTERACTION_CREATE".to_owned(),
        sequence: 1,
        data: serde_json::json!({
            "id": "100",
            "application_id": "200",
            "type": 4,
            "data": {
                "id": "300",
                "name": "admin",
                "type": 1,
                "options": [{
                    "name": "config",
                    "type": 2,
                    "options": [{
                        "name": "search",
                        "type": 1,
                        "options": [{
                            "name": "query",
                            "type": 3,
                            "value": "par",
                            "focused": focused
                        }]
                    }]
                }]
            },
            "token": "interaction-token",
            "version": 1
        }),
    })
}
