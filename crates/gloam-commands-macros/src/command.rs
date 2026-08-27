use std::{mem, sync::LazyLock};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use regex::Regex;
use syn::{
    Attribute, Error, Expr, ExprLit, ExprUnary, FnArg, GenericArgument, Ident, ItemFn, Lit, LitStr,
    Meta, Pat, PathArguments, Result, ReturnType, Token, Type, UnOp,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse2,
};

static COMMAND_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[-_'\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]{1,32}$")
        .expect("Discord chat-input command name regex must be valid")
});

const MAX_COMMAND_OPTIONS: usize = 25;
const MIN_INTEGER_VALUE: i64 = -9_007_199_254_740_991;
const MAX_INTEGER_VALUE: i64 = 9_007_199_254_740_991;
const MIN_NUMBER_VALUE: f64 = -9_007_199_254_740_992.0;
const MAX_NUMBER_VALUE: f64 = 9_007_199_254_740_992.0;
const MAX_STRING_LENGTH: u32 = 6_000;

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
                if name.replace(value).is_some() {
                    return Err(Error::new(key.span(), "duplicate `name` argument"));
                }
            } else if key == "description" {
                if description.replace(value).is_some() {
                    return Err(Error::new(key.span(), "duplicate `description` argument"));
                }
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum OptionKind {
    String,
    Boolean,
    Integer,
    Number,
    User,
    Channel,
    Role,
    Attachment,
}

#[derive(Clone, Copy)]
enum NumericBound {
    Integer(i64),
    Number(f64),
}

struct OptionParameter {
    ident: Ident,
    ty: Type,
    name: LitStr,
    description: LitStr,
    required: bool,
    min: Option<NumericBound>,
    max: Option<NumericBound>,
    min_length: Option<u32>,
    max_length: Option<u32>,
}

#[derive(Default)]
struct OptionAttributes {
    description: Option<LitStr>,
    min: Option<Expr>,
    max: Option<Expr>,
    min_length: Option<Expr>,
    max_length: Option<Expr>,
}

pub(crate) fn expand(attribute: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let args = parse2::<CommandArgs>(attribute)?;
    let mut function = parse2::<ItemFn>(item)?;
    validate_signature(&function)?;

    let function_name = function.sig.ident.clone();
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

    let context_type = context_type(&function)?.clone();
    let state_type = state_type(&context_type)?.clone();
    let options = option_parameters(&mut function)?;
    let visibility = function.vis.clone();
    let helper_suffix = function_name.unraw().to_string();
    let handler_name = format_ident!(
        "__gloam_handler_{helper_suffix}",
        span = function_name.span()
    );
    let factory_name = format_ident!(
        "__gloam_command_{helper_suffix}",
        span = function_name.span()
    );

    let option_descriptors = options.iter().map(option_descriptor_tokens);
    let option_idents = options
        .iter()
        .map(|option| &option.ident)
        .collect::<Vec<_>>();
    let option_types = options.iter().map(|option| &option.ty).collect::<Vec<_>>();
    let option_names = options
        .iter()
        .map(|option| &option.name)
        .collect::<Vec<_>>();

    let handler_body = if options.is_empty() {
        quote! { ::std::boxed::Box::pin(#function_name(ctx)) }
    } else {
        quote! {
            ::std::boxed::Box::pin(async move {
                let (#(#option_idents,)*) = {
                    let options = ::gloam_commands::CommandOptions::new(ctx.command_data());
                    (#(
                        <#option_types as ::gloam_commands::CommandOption>::extract(
                            &options,
                            #option_names,
                        )?,
                    )*)
                };
                #function_name(ctx, #(#option_idents),*).await
            })
        }
    };

    Ok(quote! {
        #function

        #[doc(hidden)]
        fn #handler_name(ctx: #context_type) -> ::gloam_commands::CommandFuture {
            #handler_body
        }

        #[doc(hidden)]
        #visibility fn #factory_name() -> ::gloam_commands::SlashCommand<#state_type> {
            static OPTIONS: &[::gloam_commands::CommandOptionDescriptor] = &[
                #(#option_descriptors),*
            ];
            static DESCRIPTOR: ::gloam_commands::CommandDescriptor =
                ::gloam_commands::CommandDescriptor::new(#command_name, #description)
                    .with_options(OPTIONS);

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

    validate_return_type(&signature.output)?;
    let _ = context_type(function)?;
    Ok(())
}

fn option_parameters(function: &mut ItemFn) -> Result<Vec<OptionParameter>> {
    let option_count = function.sig.inputs.len().saturating_sub(1);
    if option_count > MAX_COMMAND_OPTIONS {
        return Err(Error::new_spanned(
            &function.sig.inputs,
            "Discord chat-input commands support at most 25 options",
        ));
    }

    let mut options = Vec::with_capacity(option_count);
    let mut saw_optional = false;
    for argument in function.sig.inputs.iter_mut().skip(1) {
        let option = option_parameter(argument)?;
        if option.required && saw_optional {
            return Err(Error::new(
                option.ident.span(),
                "required slash-command options must appear before optional `Option<T>` parameters",
            ));
        }
        saw_optional |= !option.required;
        options.push(option);
    }
    Ok(options)
}

fn option_parameter(argument: &mut FnArg) -> Result<OptionParameter> {
    let FnArg::Typed(argument) = argument else {
        return Err(Error::new_spanned(
            argument,
            "slash-command options must be typed function parameters",
        ));
    };
    let Pat::Ident(pattern) = argument.pat.as_ref() else {
        return Err(Error::new_spanned(
            &argument.pat,
            "slash-command options require simple identifier parameter patterns",
        ));
    };
    if pattern.subpat.is_some() {
        return Err(Error::new_spanned(
            &argument.pat,
            "slash-command options require simple identifier parameter patterns",
        ));
    }

    let ident = pattern.ident.clone();
    let name = LitStr::new(&ident.unraw().to_string(), ident.span());
    validate_name(&name)?;

    let ty = argument.ty.as_ref().clone();
    let (kind, required) = option_type(&ty)?;
    let attributes = take_option_attributes(&mut argument.attrs)?;
    let description = attributes.description.ok_or_else(|| {
        Error::new(
            ident.span(),
            "slash-command options require `#[description = \"...\"]`",
        )
    })?;
    validate_description(&description)?;

    let (min, max) = numeric_bounds(kind, attributes.min.as_ref(), attributes.max.as_ref())?;
    let (min_length, max_length) = string_bounds(
        kind,
        attributes.min_length.as_ref(),
        attributes.max_length.as_ref(),
    )?;

    Ok(OptionParameter {
        ident,
        ty,
        name,
        description,
        required,
        min,
        max,
        min_length,
        max_length,
    })
}

fn take_option_attributes(attributes: &mut Vec<Attribute>) -> Result<OptionAttributes> {
    let mut parsed = OptionAttributes::default();
    let mut retained = Vec::with_capacity(attributes.len());

    for attribute in mem::take(attributes) {
        if attribute.path().is_ident("description") {
            set_once(
                &mut parsed.description,
                parse_string_attribute(&attribute, "description")?,
                &attribute,
                "description",
            )?;
        } else if attribute.path().is_ident("min") {
            set_once(
                &mut parsed.min,
                parse_expression_attribute(&attribute, "min")?,
                &attribute,
                "min",
            )?;
        } else if attribute.path().is_ident("max") {
            set_once(
                &mut parsed.max,
                parse_expression_attribute(&attribute, "max")?,
                &attribute,
                "max",
            )?;
        } else if attribute.path().is_ident("min_length") {
            set_once(
                &mut parsed.min_length,
                parse_expression_attribute(&attribute, "min_length")?,
                &attribute,
                "min_length",
            )?;
        } else if attribute.path().is_ident("max_length") {
            set_once(
                &mut parsed.max_length,
                parse_expression_attribute(&attribute, "max_length")?,
                &attribute,
                "max_length",
            )?;
        } else {
            retained.push(attribute);
        }
    }

    *attributes = retained;
    Ok(parsed)
}

fn set_once<T>(slot: &mut Option<T>, value: T, attribute: &Attribute, name: &str) -> Result<()> {
    if slot.is_some() {
        return Err(Error::new_spanned(
            attribute,
            format!("duplicate `#[{name} = ...]` attribute"),
        ));
    }
    *slot = Some(value);
    Ok(())
}

fn parse_string_attribute(attribute: &Attribute, name: &str) -> Result<LitStr> {
    let expression = parse_expression_attribute(attribute, name)?;
    let Expr::Lit(ExprLit {
        lit: Lit::Str(value),
        ..
    }) = expression
    else {
        return Err(Error::new_spanned(
            attribute,
            format!("`#[{name} = ...]` requires a string literal"),
        ));
    };
    Ok(value)
}

fn parse_expression_attribute(attribute: &Attribute, name: &str) -> Result<Expr> {
    let Meta::NameValue(meta) = &attribute.meta else {
        return Err(Error::new_spanned(
            attribute,
            format!("expected `#[{name} = ...]`"),
        ));
    };
    Ok(meta.value.clone())
}

fn option_type(ty: &Type) -> Result<(OptionKind, bool)> {
    if let Some(inner) = option_inner_type(ty)? {
        if option_inner_type(inner)?.is_some() {
            return Err(Error::new_spanned(
                ty,
                "nested `Option<Option<T>>` slash-command parameters are not supported",
            ));
        }
        return Ok((supported_option_kind(inner)?, false));
    }
    Ok((supported_option_kind(ty)?, true))
}

fn option_inner_type(ty: &Type) -> Result<Option<&Type>> {
    let Type::Path(path) = ty else {
        return Ok(None);
    };
    let Some(segment) = path.path.segments.last() else {
        return Ok(None);
    };
    if segment.ident != "Option" {
        return Ok(None);
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(Error::new_spanned(
            ty,
            "`Option` slash-command parameters require exactly one inner type",
        ));
    };
    if arguments.args.len() != 1 {
        return Err(Error::new_spanned(
            ty,
            "`Option` slash-command parameters require exactly one inner type",
        ));
    }
    let Some(GenericArgument::Type(inner)) = arguments.args.first() else {
        return Err(Error::new_spanned(
            ty,
            "`Option` slash-command parameters require an inner Rust type",
        ));
    };
    Ok(Some(inner))
}

fn supported_option_kind(ty: &Type) -> Result<OptionKind> {
    let Type::Path(path) = ty else {
        return Err(unsupported_option_type(ty));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(unsupported_option_type(ty));
    };
    if !matches!(segment.arguments, PathArguments::None) {
        return Err(unsupported_option_type(ty));
    }

    match segment.ident.to_string().as_str() {
        "String" => Ok(OptionKind::String),
        "bool" => Ok(OptionKind::Boolean),
        "i64" => Ok(OptionKind::Integer),
        "f64" => Ok(OptionKind::Number),
        "UserId" => Ok(OptionKind::User),
        "ChannelId" => Ok(OptionKind::Channel),
        "RoleId" => Ok(OptionKind::Role),
        "AttachmentId" => Ok(OptionKind::Attachment),
        _ => Err(unsupported_option_type(ty)),
    }
}

fn unsupported_option_type(ty: &Type) -> Error {
    Error::new_spanned(
        ty,
        "unsupported slash-command option type; expected `String`, `bool`, `i64`, `f64`, `UserId`, `ChannelId`, `RoleId`, `AttachmentId`, or `Option<T>` of one of those types",
    )
}

fn numeric_bounds(
    kind: OptionKind,
    min_expression: Option<&Expr>,
    max_expression: Option<&Expr>,
) -> Result<(Option<NumericBound>, Option<NumericBound>)> {
    if min_expression.is_none() && max_expression.is_none() {
        return Ok((None, None));
    }

    match kind {
        OptionKind::Integer => {
            let min = min_expression
                .map(parse_integer_bound)
                .transpose()?
                .map(NumericBound::Integer);
            let max = max_expression
                .map(parse_integer_bound)
                .transpose()?
                .map(NumericBound::Integer);
            if let (Some(NumericBound::Integer(min)), Some(NumericBound::Integer(max))) = (min, max)
                && min > max
            {
                return Err(Error::new_spanned(
                    max_expression.expect("maximum expression is present"),
                    "integer `min` cannot be greater than `max`",
                ));
            }
            Ok((min, max))
        }
        OptionKind::Number => {
            let min = min_expression
                .map(parse_number_bound)
                .transpose()?
                .map(NumericBound::Number);
            let max = max_expression
                .map(parse_number_bound)
                .transpose()?
                .map(NumericBound::Number);
            if let (Some(NumericBound::Number(min)), Some(NumericBound::Number(max))) = (min, max)
                && min > max
            {
                return Err(Error::new_spanned(
                    max_expression.expect("maximum expression is present"),
                    "number `min` cannot be greater than `max`",
                ));
            }
            Ok((min, max))
        }
        _ => Err(Error::new_spanned(
            min_expression
                .or(max_expression)
                .expect("numeric constraint is present"),
            "`#[min = ...]` and `#[max = ...]` are only supported for `i64` and `f64` options",
        )),
    }
}

fn string_bounds(
    kind: OptionKind,
    min_expression: Option<&Expr>,
    max_expression: Option<&Expr>,
) -> Result<(Option<u32>, Option<u32>)> {
    if min_expression.is_none() && max_expression.is_none() {
        return Ok((None, None));
    }
    if kind != OptionKind::String {
        return Err(Error::new_spanned(
            min_expression
                .or(max_expression)
                .expect("string-length constraint is present"),
            "`#[min_length = ...]` and `#[max_length = ...]` are only supported for `String` options",
        ));
    }

    let min = min_expression.map(parse_length).transpose()?;
    let max = max_expression.map(parse_length).transpose()?;
    if let Some(value) = min
        && value > MAX_STRING_LENGTH
    {
        return Err(Error::new_spanned(
            min_expression.expect("minimum expression is present"),
            "Discord string `min_length` must be between 0 and 6000",
        ));
    }
    if let Some(value) = max
        && !(1..=MAX_STRING_LENGTH).contains(&value)
    {
        return Err(Error::new_spanned(
            max_expression.expect("maximum expression is present"),
            "Discord string `max_length` must be between 1 and 6000",
        ));
    }
    if let (Some(min), Some(max)) = (min, max)
        && min > max
    {
        return Err(Error::new_spanned(
            max_expression.expect("maximum expression is present"),
            "string `min_length` cannot be greater than `max_length`",
        ));
    }
    Ok((min, max))
}

fn parse_integer_bound(expression: &Expr) -> Result<i64> {
    let (negative, literal) = signed_literal(expression)?;
    let Lit::Int(literal) = literal else {
        return Err(Error::new_spanned(
            expression,
            "integer option bounds require integer literals",
        ));
    };
    let magnitude = literal.base10_digits().parse::<i128>().map_err(|_| {
        Error::new_spanned(
            expression,
            "integer option bound is outside the supported range",
        )
    })?;
    let value = if negative { -magnitude } else { magnitude };
    if value < i128::from(MIN_INTEGER_VALUE) || value > i128::from(MAX_INTEGER_VALUE) {
        return Err(Error::new_spanned(
            expression,
            "Discord integer option bounds must be between -9007199254740991 and 9007199254740991",
        ));
    }
    Ok(value as i64)
}

fn parse_number_bound(expression: &Expr) -> Result<f64> {
    let (negative, literal) = signed_literal(expression)?;
    let magnitude = match literal {
        Lit::Float(literal) => literal.base10_parse::<f64>(),
        Lit::Int(literal) => literal.base10_digits().parse::<f64>(),
        _ => {
            return Err(Error::new_spanned(
                expression,
                "number option bounds require numeric literals",
            ));
        }
    }
    .map_err(|_| Error::new_spanned(expression, "invalid number option bound"))?;
    let value = if negative { -magnitude } else { magnitude };
    if !value.is_finite() || !(MIN_NUMBER_VALUE..=MAX_NUMBER_VALUE).contains(&value) {
        return Err(Error::new_spanned(
            expression,
            "Discord number option bounds must be between -9007199254740992 and 9007199254740992",
        ));
    }
    Ok(value)
}

fn parse_length(expression: &Expr) -> Result<u32> {
    let Expr::Lit(ExprLit {
        lit: Lit::Int(literal),
        ..
    }) = expression
    else {
        return Err(Error::new_spanned(
            expression,
            "string length constraints require non-negative integer literals",
        ));
    };
    literal.base10_parse::<u32>().map_err(|_| {
        Error::new_spanned(
            expression,
            "string length constraint is outside the supported range",
        )
    })
}

fn signed_literal(expression: &Expr) -> Result<(bool, &Lit)> {
    match expression {
        Expr::Lit(literal) => Ok((false, &literal.lit)),
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        }) => {
            let Expr::Lit(literal) = expr.as_ref() else {
                return Err(Error::new_spanned(
                    expression,
                    "numeric option bounds require literals",
                ));
            };
            Ok((true, &literal.lit))
        }
        _ => Err(Error::new_spanned(
            expression,
            "numeric option bounds require literals",
        )),
    }
}

fn option_descriptor_tokens(option: &OptionParameter) -> TokenStream {
    let name = &option.name;
    let description = &option.description;
    let ty = &option.ty;
    let required = option.required;
    let mut descriptor = quote! {
        ::gloam_commands::CommandOptionDescriptor::new(
            #name,
            #description,
            <#ty as ::gloam_commands::CommandOption>::KIND,
            #required,
        )
    };

    if let Some(min) = option.min {
        descriptor = match min {
            NumericBound::Integer(value) => quote! { (#descriptor).min_integer(#value) },
            NumericBound::Number(value) => quote! { (#descriptor).min_number(#value) },
        };
    }
    if let Some(max) = option.max {
        descriptor = match max {
            NumericBound::Integer(value) => quote! { (#descriptor).max_integer(#value) },
            NumericBound::Number(value) => quote! { (#descriptor).max_number(#value) },
        };
    }
    if let Some(value) = option.min_length {
        descriptor = quote! { (#descriptor).min_length(#value) };
    }
    if let Some(value) = option.max_length {
        descriptor = quote! { (#descriptor).max_length(#value) };
    }
    descriptor
}

fn validate_return_type(output: &ReturnType) -> Result<()> {
    let ReturnType::Type(_, output) = output else {
        return Err(Error::new_spanned(
            output,
            "slash commands must return `Result<()>`",
        ));
    };
    let Type::Path(result) = output.as_ref() else {
        return Err(Error::new_spanned(
            output,
            "slash commands must return `Result<()>`",
        ));
    };
    let Some(segment) = result.path.segments.last() else {
        return Err(Error::new_spanned(
            output,
            "slash commands must return `Result<()>`",
        ));
    };
    if segment.ident != "Result" {
        return Err(Error::new_spanned(
            output,
            "slash commands must return `Result<()>`",
        ));
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(Error::new_spanned(
            output,
            "slash commands must return `Result<()>`",
        ));
    };
    let is_unit_result = matches!(
        arguments.args.first(),
        Some(GenericArgument::Type(Type::Tuple(unit)))
            if arguments.args.len() == 1 && unit.elems.is_empty()
    );
    if !is_unit_result {
        return Err(Error::new_spanned(
            output,
            "slash commands must return `Result<()>`",
        ));
    }
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
