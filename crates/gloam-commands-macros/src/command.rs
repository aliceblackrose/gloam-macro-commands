use std::sync::LazyLock;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use regex::Regex;
use syn::{
    Error, FnArg, GenericArgument, Ident, ItemFn, LitStr, PathArguments, Result, ReturnType, Token,
    Type,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse2,
};

static COMMAND_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[-_'\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]{1,32}$")
        .expect("Discord chat-input command name regex must be valid")
});

struct CommandArgs {
    name: Option<LitStr>,
    description: Option<LitStr>,
}

impl Parse for CommandArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut name = None;
        let mut description = None;

        while !input.is_empty() {
            let key = input.call(Ident::parse_any)?;
            input.parse::<Token![=]>()?;
            let value = input.parse::<LitStr>()?;

            if key == "name" {
                if name.is_some() {
                    return Err(Error::new(key.span(), "duplicate `name` argument"));
                }
                name = Some(value);
            } else if key == "description" {
                if description.is_some() {
                    return Err(Error::new(key.span(), "duplicate `description` argument"));
                }
                description = Some(value);
            } else {
                return Err(Error::new(
                    key.span(),
                    "unsupported command argument; expected `name` or `description`",
                ));
            }

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
        }

        Ok(Self { name, description })
    }
}

pub(crate) fn expand(attribute: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let args = parse2::<CommandArgs>(attribute)?;
    let function = parse2::<ItemFn>(item)?;
    validate_signature(&function)?;

    let function_name = &function.sig.ident;
    let command_name = args
        .name
        .unwrap_or_else(|| LitStr::new(&function_name.unraw().to_string(), function_name.span()));
    let description = args.description.ok_or_else(|| {
        Error::new(
            function_name.span(),
            "slash commands require `description = \"...\"`",
        )
    })?;

    validate_name(&command_name)?;
    validate_description(&description)?;

    let context_type = context_type(&function)?;
    let state_type = state_type(context_type)?;
    let visibility = &function.vis;
    let helper_suffix = function_name.unraw().to_string();
    let handler_name = format_ident!(
        "__gloam_handler_{helper_suffix}",
        span = function_name.span()
    );
    let factory_name = format_ident!(
        "__gloam_command_{helper_suffix}",
        span = function_name.span()
    );

    Ok(quote! {
        #function

        #[doc(hidden)]
        fn #handler_name(ctx: #context_type) -> ::gloam_commands::CommandFuture {
            ::std::boxed::Box::pin(#function_name(ctx))
        }

        #[doc(hidden)]
        #visibility fn #factory_name() -> ::gloam_commands::SlashCommand<#state_type> {
            static DESCRIPTOR: ::gloam_commands::CommandDescriptor =
                ::gloam_commands::CommandDescriptor::new(#command_name, #description);

            ::gloam_commands::SlashCommand::new(&DESCRIPTOR, #handler_name)
        }
    })
}

fn validate_signature(function: &ItemFn) -> Result<()> {
    let signature = &function.sig;

    if signature.asyncness.is_none() {
        return Err(Error::new_spanned(
            signature.fn_token,
            "slash commands must be declared with `async fn`",
        ));
    }

    if signature.constness.is_some() {
        return Err(Error::new_spanned(
            signature.constness,
            "slash commands cannot be `const`",
        ));
    }

    if signature.unsafety.is_some() {
        return Err(Error::new_spanned(
            signature.unsafety,
            "slash commands cannot be `unsafe`",
        ));
    }

    if signature.abi.is_some() {
        return Err(Error::new_spanned(
            &signature.abi,
            "slash commands cannot declare an explicit ABI",
        ));
    }

    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &signature.generics,
            "generic slash-command handlers are not supported",
        ));
    }

    if signature.variadic.is_some() {
        return Err(Error::new_spanned(
            &signature.variadic,
            "variadic slash-command handlers are not supported",
        ));
    }

    if signature.inputs.len() != 1 {
        return Err(Error::new_spanned(
            &signature.inputs,
            "Phase 2 slash commands accept exactly one parameter: `Context<D>`; typed command options are added in Phase 5",
        ));
    }

    if matches!(signature.output, ReturnType::Default) {
        return Err(Error::new_spanned(
            &signature.output,
            "slash commands must declare a result return type",
        ));
    }

    let _ = context_type(function)?;
    Ok(())
}

fn context_type(function: &ItemFn) -> Result<&Type> {
    let Some(argument) = function.sig.inputs.first() else {
        return Err(Error::new(
            function.sig.ident.span(),
            "slash commands require a `Context<D>` parameter",
        ));
    };

    let FnArg::Typed(argument) = argument else {
        return Err(Error::new_spanned(
            argument,
            "slash commands must be free functions whose first parameter is `Context<D>`",
        ));
    };

    let Type::Path(context) = argument.ty.as_ref() else {
        return Err(Error::new_spanned(
            &argument.ty,
            "slash-command context parameter must have type `Context<D>`",
        ));
    };

    let Some(segment) = context.path.segments.last() else {
        return Err(Error::new_spanned(
            &argument.ty,
            "slash-command context parameter must have type `Context<D>`",
        ));
    };

    if segment.ident != "Context" {
        return Err(Error::new_spanned(
            &argument.ty,
            "slash-command context parameter must have type `Context<D>`",
        ));
    }

    Ok(argument.ty.as_ref())
}

fn state_type(context_type: &Type) -> Result<&Type> {
    let Type::Path(context) = context_type else {
        unreachable!("context_type validates Type::Path");
    };
    let segment = context
        .path
        .segments
        .last()
        .expect("context_type validates a non-empty path");

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(Error::new_spanned(
            context_type,
            "`Context` requires exactly one application-state type parameter",
        ));
    };

    if arguments.args.len() != 1 {
        return Err(Error::new_spanned(
            arguments,
            "`Context` requires exactly one application-state type parameter",
        ));
    }

    let Some(GenericArgument::Type(state_type)) = arguments.args.first() else {
        return Err(Error::new_spanned(
            arguments,
            "`Context` requires an application-state type parameter",
        ));
    };

    Ok(state_type)
}

fn validate_name(name: &LitStr) -> Result<()> {
    let value = name.value();
    if !COMMAND_NAME.is_match(&value) {
        return Err(Error::new(
            name.span(),
            "invalid Discord chat-input command name; expected 1-32 characters matching Discord's application-command naming rules",
        ));
    }

    if value.to_lowercase() != value {
        return Err(Error::new(
            name.span(),
            "Discord chat-input command names must use lowercase variants when available",
        ));
    }

    Ok(())
}

fn validate_description(description: &LitStr) -> Result<()> {
    let length = description.value().chars().count();
    if !(1..=100).contains(&length) {
        return Err(Error::new(
            description.span(),
            "Discord slash-command descriptions must contain between 1 and 100 characters",
        ));
    }

    Ok(())
}
