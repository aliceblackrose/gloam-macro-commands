use gloamwire::model::{
    ApplicationCommandInteractionData, ApplicationCommandInteractionDataOption,
    ApplicationCommandInteractionValue, ApplicationCommandNumericValue, ApplicationCommandOptionType,
    AttachmentId, ChannelId,
    RoleId, UserId,
};

use crate::{CommandChoiceDescriptor, CommandChoiceValue, CommandOptionDescriptor, Error, Result};

/// Submitted scalar options for one resolved Discord chat-input command path.
///
/// Generated command adapters use this view to extract typed Rust parameters
/// from the leaf option scope that dispatch already resolved.
pub struct CommandOptions<'a> {
    options: &'a [ApplicationCommandInteractionDataOption],
}

impl<'a> CommandOptions<'a> {
    /// Creates an option view over top-level parsed application-command data.
    #[must_use]
    pub fn new(data: &'a ApplicationCommandInteractionData) -> Self {
        Self::from_slice(&data.options)
    }

    /// Creates an option view over an already-resolved leaf option slice.
    #[must_use]
    pub const fn from_slice(options: &'a [ApplicationCommandInteractionDataOption]) -> Self {
        Self { options }
    }

    /// Returns a submitted option by its registered name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&'a ApplicationCommandInteractionDataOption> {
        self.options.iter().find(|option| option.name == name)
    }

    pub(crate) fn validate(&self, descriptors: &[CommandOptionDescriptor]) -> Result<()> {
        // Discord allows at most 25 options, so a bitset avoids a temporary map.
        let mut seen = 0u32;
        for option in self.options {
            let Some(index) = descriptors.iter().position(|d| d.name == option.name) else {
                return Err(Error::InvalidSubmittedOptions);
            };
            let descriptor = &descriptors[index];
            let bit = 1 << index;
            if seen & bit != 0
                || option.kind != descriptor.kind
                || !option.options.is_empty()
                || option.focused == Some(true)
            {
                return Err(Error::InvalidSubmittedOptions);
            }
            seen |= bit;
            validate_value(option, descriptor)?;
        }
        for (index, descriptor) in descriptors.iter().enumerate() {
            if descriptor.required && seen & (1 << index) == 0 {
                return Err(Error::MissingOption(descriptor.name));
            }
        }
        Ok(())
    }
}

fn numeric_bound(bound: ApplicationCommandNumericValue) -> f64 {
    match bound {
        ApplicationCommandNumericValue::Integer(value) => value as f64,
        ApplicationCommandNumericValue::Number(value) => value,
    }
}

fn validate_value(
    option: &ApplicationCommandInteractionDataOption,
    descriptor: &CommandOptionDescriptor,
) -> Result<()> {
    use ApplicationCommandInteractionValue as Value;
    use ApplicationCommandOptionType as Kind;
    let invalid = || invalid_option(descriptor.name, "registered option");
    let value = option.value.as_ref().ok_or_else(invalid)?;
    let number = match (descriptor.kind, value) {
        (Kind::STRING, Value::String(value)) => {
            let length = value.chars().count();
            if length > 6000
                || descriptor.min_length.is_some_and(|min| length < min as usize)
                || descriptor.max_length.is_some_and(|max| length > max as usize)
            {
                return Err(invalid());
            }
            None
        }
        (Kind::BOOLEAN, Value::Boolean(_)) => None,
        (Kind::INTEGER, Value::Integer(value))
            if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(value) =>
        {
            Some(*value as f64)
        }
        (Kind::NUMBER, Value::Integer(value))
            if (-9_007_199_254_740_992..=9_007_199_254_740_992).contains(value) =>
        {
            Some(*value as f64)
        }
        (Kind::NUMBER, Value::Number(value))
            if (-9_007_199_254_740_992.0..=9_007_199_254_740_992.0).contains(value) =>
        {
            Some(*value)
        }
        (Kind::USER | Kind::ROLE | Kind::CHANNEL | Kind::ATTACHMENT | Kind::MENTIONABLE,
            Value::String(value)) if value.parse::<u64>().is_ok_and(|id| id != 0) => None,
        _ => return Err(invalid()),
    };
    if let Some(number) = number
        && (descriptor.min_value.is_some_and(|min| number < numeric_bound(min))
            || descriptor.max_value.is_some_and(|max| number > numeric_bound(max)))
    {
        return Err(invalid());
    }
    if !descriptor.choices.is_empty()
        && !descriptor.choices.iter().any(|choice| match (choice.value, value) {
            (CommandChoiceValue::String(expected), Value::String(actual)) => expected == actual,
            (CommandChoiceValue::Integer(expected), Value::Integer(actual)) => expected == *actual,
            (CommandChoiceValue::Number(expected), _) => number == Some(expected),
            _ => false,
        })
    {
        return Err(Error::InvalidChoice { name: descriptor.name });
    }
    Ok(())
}

/// Rust type that can be extracted from a Discord chat-input command option.
pub trait CommandOption: Sized {
    /// Discord application-command option kind represented by this Rust type.
    const KIND: ApplicationCommandOptionType;

    /// Extracts this value from one submitted option name.
    fn extract(options: &CommandOptions<'_>, name: &'static str) -> Result<Self>;
}

/// Typed command option backed by a fixed Discord choice list.
///
/// Implementations are normally generated with [`macro@crate::CommandChoice`].
pub trait CommandChoice: CommandOption {
    /// Static choices registered for this typed option.
    const CHOICES: &'static [CommandChoiceDescriptor];
}

impl CommandOption for String {
    const KIND: ApplicationCommandOptionType = ApplicationCommandOptionType::STRING;

    fn extract(options: &CommandOptions<'_>, name: &'static str) -> Result<Self> {
        let option = required_option(options, name)?;
        validate_kind(option, name, Self::KIND, "String")?;

        match option.value.as_ref() {
            Some(ApplicationCommandInteractionValue::String(value)) => Ok(value.clone()),
            _ => Err(invalid_option(name, "String")),
        }
    }
}

macro_rules! impl_copy_option {
    ($type:ty, $kind:path, $variant:path, $expected:literal) => {
        impl CommandOption for $type {
            const KIND: ApplicationCommandOptionType = $kind;

            fn extract(options: &CommandOptions<'_>, name: &'static str) -> Result<Self> {
                let option = required_option(options, name)?;
                validate_kind(option, name, Self::KIND, $expected)?;

                match option.value.as_ref() {
                    Some($variant(value)) => Ok(*value),
                    _ => Err(invalid_option(name, $expected)),
                }
            }
        }
    };
}

impl_copy_option!(
    bool,
    ApplicationCommandOptionType::BOOLEAN,
    ApplicationCommandInteractionValue::Boolean,
    "bool"
);
impl_copy_option!(
    i64,
    ApplicationCommandOptionType::INTEGER,
    ApplicationCommandInteractionValue::Integer,
    "i64"
);
impl CommandOption for f64 {
    const KIND: ApplicationCommandOptionType = ApplicationCommandOptionType::NUMBER;

    fn extract(options: &CommandOptions<'_>, name: &'static str) -> Result<Self> {
        let option = required_option(options, name)?;
        validate_kind(option, name, Self::KIND, "f64")?;
        let value = match option.value.as_ref() {
            Some(ApplicationCommandInteractionValue::Number(value)) => *value,
            Some(ApplicationCommandInteractionValue::Integer(value))
                if (-9_007_199_254_740_992..=9_007_199_254_740_992).contains(value) =>
            {
                *value as f64
            }
            _ => return Err(invalid_option(name, "f64")),
        };
        if !(-9_007_199_254_740_992.0..=9_007_199_254_740_992.0).contains(&value) {
            return Err(invalid_option(name, "f64"));
        }
        Ok(value)
    }
}

macro_rules! impl_id_option {
    ($type:ty, $kind:path, $expected:literal) => {
        impl CommandOption for $type {
            const KIND: ApplicationCommandOptionType = $kind;

            fn extract(options: &CommandOptions<'_>, name: &'static str) -> Result<Self> {
                let option = required_option(options, name)?;
                validate_kind(option, name, Self::KIND, $expected)?;

                let Some(ApplicationCommandInteractionValue::String(value)) = option.value.as_ref()
                else {
                    return Err(invalid_option(name, $expected));
                };

                value.parse().map_err(|_| invalid_option(name, $expected))
            }
        }
    };
}

impl_id_option!(
    UserId,
    ApplicationCommandOptionType::USER,
    "gloamwire::model::UserId"
);
impl_id_option!(
    ChannelId,
    ApplicationCommandOptionType::CHANNEL,
    "gloamwire::model::ChannelId"
);
impl_id_option!(
    RoleId,
    ApplicationCommandOptionType::ROLE,
    "gloamwire::model::RoleId"
);
impl_id_option!(
    AttachmentId,
    ApplicationCommandOptionType::ATTACHMENT,
    "gloamwire::model::AttachmentId"
);

impl<T> CommandOption for Option<T>
where
    T: CommandOption,
{
    const KIND: ApplicationCommandOptionType = T::KIND;

    fn extract(options: &CommandOptions<'_>, name: &'static str) -> Result<Self> {
        if options.get(name).is_none() {
            return Ok(None);
        }

        T::extract(options, name).map(Some)
    }
}

impl<T> CommandChoice for Option<T>
where
    T: CommandChoice,
{
    const CHOICES: &'static [CommandChoiceDescriptor] = T::CHOICES;
}

fn required_option<'a>(
    options: &CommandOptions<'a>,
    name: &'static str,
) -> Result<&'a ApplicationCommandInteractionDataOption> {
    options.get(name).ok_or(Error::MissingOption(name))
}

fn validate_kind(
    option: &ApplicationCommandInteractionDataOption,
    name: &'static str,
    expected_kind: ApplicationCommandOptionType,
    expected: &'static str,
) -> Result<()> {
    if option.kind != expected_kind {
        return Err(invalid_option(name, expected));
    }

    Ok(())
}

const fn invalid_option(name: &'static str, expected: &'static str) -> Error {
    Error::InvalidOption { name, expected }
}

// Dispatch has already validated that every traversed group contains exactly
// one submitted branch. Reuse the original immutable interaction allocation.
pub(crate) fn leaf_options(
    data: &ApplicationCommandInteractionData,
    depth: usize,
) -> &[ApplicationCommandInteractionDataOption] {
    let mut options = data.options.as_slice();
    for _ in 0..depth {
        options = &options[0].options;
    }
    options
}

#[cfg(test)]
mod tests {
    use gloamwire::model::{
        ApplicationCommandInteractionData, ApplicationCommandInteractionDataOption,
        ApplicationCommandInteractionValue, ApplicationCommandOptionType, ApplicationCommandType,
        CommandId, UserId,
    };

    use super::{CommandOption, CommandOptions};
    use crate::{CommandOptionDescriptor, Error};

    #[test]
    fn extracts_scalar_values() {
        let data = command_data(vec![
            option(
                "text",
                ApplicationCommandOptionType::STRING,
                ApplicationCommandInteractionValue::String("hello".to_owned()),
            ),
            option(
                "enabled",
                ApplicationCommandOptionType::BOOLEAN,
                ApplicationCommandInteractionValue::Boolean(true),
            ),
            option(
                "count",
                ApplicationCommandOptionType::INTEGER,
                ApplicationCommandInteractionValue::Integer(42),
            ),
            option(
                "ratio",
                ApplicationCommandOptionType::NUMBER,
                ApplicationCommandInteractionValue::Number(1.5),
            ),
        ]);
        let options = CommandOptions::new(&data);

        assert_eq!(String::extract(&options, "text").expect("string"), "hello");
        assert!(bool::extract(&options, "enabled").expect("bool"));
        assert_eq!(i64::extract(&options, "count").expect("integer"), 42);
        assert_eq!(f64::extract(&options, "ratio").expect("number"), 1.5);
    }

    #[test]
    fn extracts_integer_encoded_number_from_json() {
        let submitted: ApplicationCommandInteractionDataOption =
            serde_json::from_value(serde_json::json!({"name": "ratio", "type": 10, "value": 42}))
                .expect("valid option");
        let options = CommandOptions::from_slice(std::slice::from_ref(&submitted));
        assert_eq!(f64::extract(&options, "ratio").expect("number"), 42.0);
    }

    #[test]
    fn rejects_out_of_bounds_and_duplicate_submissions() {
        let descriptor = CommandOptionDescriptor::new(
            "count",
            "Count",
            ApplicationCommandOptionType::INTEGER,
            true,
        )
        .max_integer(3);
        let too_large = option(
            "count",
            ApplicationCommandOptionType::INTEGER,
            ApplicationCommandInteractionValue::Integer(4),
        );
        assert!(matches!(
            CommandOptions::from_slice(&[too_large]).validate(&[descriptor]),
            Err(Error::InvalidOption { .. })
        ));
        let valid = option(
            "count",
            ApplicationCommandOptionType::INTEGER,
            ApplicationCommandInteractionValue::Integer(3),
        );
        assert!(matches!(
            CommandOptions::from_slice(&[valid.clone(), valid]).validate(&[descriptor]),
            Err(Error::InvalidSubmittedOptions)
        ));
        assert!(matches!(
            CommandOptions::from_slice(&[]).validate(&[descriptor]),
            Err(Error::MissingOption("count"))
        ));
    }

    #[test]
    fn extracts_from_resolved_nested_scope() {
        let nested = vec![option(
            "count",
            ApplicationCommandOptionType::INTEGER,
            ApplicationCommandInteractionValue::Integer(7),
        )];
        let options = CommandOptions::from_slice(&nested);

        assert_eq!(i64::extract(&options, "count").expect("nested integer"), 7);
    }

    #[test]
    fn optional_values_allow_missing_options() {
        let data = command_data(Vec::new());
        let options = CommandOptions::new(&data);

        assert_eq!(
            Option::<String>::extract(&options, "query").expect("optional"),
            None
        );
        assert!(matches!(
            String::extract(&options, "query"),
            Err(Error::MissingOption("query"))
        ));
    }

    #[test]
    fn extracts_typed_snowflake_ids() {
        let data = command_data(vec![option(
            "user",
            ApplicationCommandOptionType::USER,
            ApplicationCommandInteractionValue::String("123".to_owned()),
        )]);
        let options = CommandOptions::new(&data);

        assert_eq!(
            UserId::extract(&options, "user").expect("user id"),
            UserId::new(123)
        );
    }

    #[test]
    fn rejects_mismatched_option_kinds() {
        let data = command_data(vec![option(
            "count",
            ApplicationCommandOptionType::STRING,
            ApplicationCommandInteractionValue::String("42".to_owned()),
        )]);
        let options = CommandOptions::new(&data);

        assert!(matches!(
            i64::extract(&options, "count"),
            Err(Error::InvalidOption {
                name: "count",
                expected: "i64"
            })
        ));
    }

    fn command_data(
        options: Vec<ApplicationCommandInteractionDataOption>,
    ) -> ApplicationCommandInteractionData {
        ApplicationCommandInteractionData {
            id: CommandId::new(1),
            name: "test".to_owned(),
            kind: ApplicationCommandType::CHAT_INPUT,
            resolved: None,
            options,
            guild_id: None,
            target_id: None,
        }
    }

    fn option(
        name: &str,
        kind: ApplicationCommandOptionType,
        value: ApplicationCommandInteractionValue,
    ) -> ApplicationCommandInteractionDataOption {
        ApplicationCommandInteractionDataOption {
            name: name.to_owned(),
            kind,
            value: Some(value),
            options: Vec::new(),
            focused: None,
        }
    }
}
