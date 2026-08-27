use std::sync::{
    Mutex,
    atomic::{AtomicI64, Ordering},
};

use gloam_commands::prelude::*;
use gloamwire::{
    RestClient,
    gateway::{DispatchEvent, GatewayEvent},
};

struct State {
    count: AtomicI64,
    path: Mutex<Vec<&'static str>>,
}

impl State {
    fn new() -> Self {
        Self {
            count: AtomicI64::new(0),
            path: Mutex::new(Vec::new()),
        }
    }
}

#[group(description = "Administration commands")]
mod admin {
    use super::*;

    #[group(description = "Configuration commands")]
    mod config {
        use super::*;

        #[command(description = "Set a value")]
        async fn set(
            ctx: Context<State>,
            #[description = "Configured value"] count: i64,
        ) -> Result<()> {
            ctx.data().count.store(count, Ordering::SeqCst);
            *ctx.data().path.lock().expect("path mutex") = ctx.command_path().to_vec();
            Ok(())
        }
    }
}

#[tokio::test]
async fn macro_group_dispatches_nested_typed_leaf() -> Result<()> {
    let framework = Framework::builder(State::new())
        .commands(commands![admin])
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
                "name": "admin",
                "type": 1,
                "options": [{
                    "name": "config",
                    "type": 2,
                    "options": [{
                        "name": "set",
                        "type": 1,
                        "options": [{"name":"count","type":4,"value":7}]
                    }]
                }]
            },
            "token": "interaction-token",
            "version": 1
        }),
    });

    let DispatchOutcome::Spawned(task) = framework.dispatch(&rest, &event)? else {
        panic!("expected nested command to spawn");
    };
    task.join().await?;

    assert_eq!(framework.data().count.load(Ordering::SeqCst), 7);
    assert_eq!(
        framework.data().path.lock().expect("path mutex").as_slice(),
        ["admin", "config", "set"]
    );
    Ok(())
}
