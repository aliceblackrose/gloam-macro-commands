#![allow(dead_code)]

use gloam_commands::{CommandChoiceValue, Context, Result, command, commands};

#[derive(Debug, Clone, Copy, PartialEq, Eq, gloam_commands::CommandChoice)]
enum Mode {
    #[choice(name = "Fast", value = "fast")]
    Fast,
    #[choice(name = "Safe")]
    Safe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, gloam_commands::CommandChoice)]
enum Level {
    #[choice(name = "Low", value = 1)]
    Low,
    #[choice(name = "High", value = 2)]
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, gloam_commands::CommandChoice)]
enum Ratio {
    #[choice(name = "Half", value = 0.5)]
    Half,
    #[choice(name = "Full", value = 1)]
    Full,
}

#[command(description = "Exercise command choices")]
async fn choose(
    _ctx: Context<()>,
    #[description = "Execution mode"]
    #[choice]
    mode: Mode,
    #[description = "Typed ratio"]
    #[choice]
    ratio: Ratio,
    #[description = "Scale"]
    #[choice(name = "Half", value = 0.5)]
    #[choice(name = "Full", value = 1.0)]
    scale: f64,
    #[description = "Optional level"]
    #[choice]
    level: Option<Level>,
) -> Result<()> {
    let _ = (mode, ratio, scale, level);
    Ok(())
}

fn main() {
    let commands = commands![choose];
    let options = commands[0].descriptor().options;

    assert_eq!(options.len(), 4);
    assert_eq!(options[0].choices.len(), 2);
    assert_eq!(
        options[0].choices[0].value,
        CommandChoiceValue::String("fast")
    );
    assert_eq!(
        options[0].choices[1].value,
        CommandChoiceValue::String("Safe")
    );
    assert_eq!(options[1].choices.len(), 2);
    assert_eq!(options[1].choices[0].value, CommandChoiceValue::Number(0.5));
    assert_eq!(options[1].choices[1].value, CommandChoiceValue::Number(1.0));
    assert_eq!(options[2].choices.len(), 2);
    assert_eq!(options[3].choices.len(), 2);
    assert!(!options[3].required);
}
