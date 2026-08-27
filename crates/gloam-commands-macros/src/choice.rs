use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DataEnum, DeriveInput, Error, Expr, ExprLit, ExprUnary, Fields, Ident, Lit,
    LitStr, Result, Token, UnOp,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse2,
};

use crate::command::{parse_integer_bound, parse_number_bound};

const MAX_CHOICES: usize = 25;
const MAX_CHOICE_STRING_VALUE: usize = 100;

struct ChoiceArgs {
    name: Option<LitStr>,
    value: Option<Expr>,
}

impl Parse for ChoiceArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut name = None;
        let mut value = None;

        while !input.is_empty() {
            let key = input.call(Ident::parse_any)?;
            input.parse::<Token![=]>()?;

            if key == "name" {
                let literal = input.parse::<LitStr>()?;
                if name.replace(literal).is_some() {
                    return Err(Error::new(key.span(), "duplicate `name` argument"));
                }
            } else if key == "value" {
                let expression = input.parse::<Expr>()?;
                if value.replace(expression).is_some() {
                    return Err(Error::new(key.span(), "duplicate `value` argument"));
                }
            } else {
                return Err(Error::new(
                    key.span(),
                    "unsupported choice argument; expected `name` or `value`",
                ));
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(Self { name, value })
    }
}

enum ParsedValue {
    String(LitStr),
    Integer(i64),
    Number(f64),
}

struct VariantChoice {
    variant: Ident,
    name: LitStr,
    value: Option<ParsedValue>,
}

pub(crate) fn expand(input: TokenStream) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(input)?;
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &input.generics,
            "generic command-choice enums are not supported",
        ));
    }

    let Data::Enum(data) = &input.data else {
        return Err(Error::new_spanned(
            &input.ident,
            "`CommandChoice` can only be derived for enums",
        ));
    };
    let choices = parse_variants(data)?;
    let kind = infer_kind(&choices)?;
    validate_duplicates(&choices, kind)?;

    let enum_ident = &input.ident;
    let descriptor_tokens = choices
        .iter()
        .map(|choice| descriptor_tokens(choice, kind))
        .collect::<Result<Vec<_>>>()?;
    let extraction = extraction_tokens(&choices, kind);
    let option_kind = option_kind_tokens(kind);

    Ok(quote! {
        impl ::gloam_commands::CommandOption for #enum_ident {
            const KIND: ::gloam_commands::__private::ApplicationCommandOptionType = #option_kind;

            fn extract(
                options: &::gloam_commands::CommandOptions<'_>,
                name: &'static str,
            ) -> ::gloam_commands::Result<Self> {
                #extraction
            }
        }

        impl ::gloam_commands::CommandChoice for #enum_ident {
            const CHOICES: &'static [::gloam_commands::CommandChoiceDescriptor] = &[
                #(#descriptor_tokens),*
            ];
        }
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChoiceKind {
    String,
    Integer,
    Number,
}

fn parse_variants(data: &DataEnum) -> Result<Vec<VariantChoice>> {
    if data.variants.is_empty() {
        return Err(Error::new_spanned(
            data.enum_token,
            "command-choice enums cannot be empty",
        ));
    }
    if data.variants.len() > MAX_CHOICES {
        return Err(Error::new_spanned(
            &data.variants,
            "Discord command options support at most 25 choices",
        ));
    }

    data.variants
        .iter()
        .map(|variant| {
            if !matches!(variant.fields, Fields::Unit) {
                return Err(Error::new_spanned(
                    &variant.fields,
                    "command-choice enum variants must be unit variants",
                ));
            }

            let attribute = choice_attribute(&variant.attrs)?.ok_or_else(|| {
                Error::new(
                    variant.ident.span(),
                    "command-choice variants require `#[choice(name = \"...\")]`",
                )
            })?;
            let args = attribute.parse_args::<ChoiceArgs>()?;
            let name = args.name.ok_or_else(|| {
                Error::new_spanned(attribute, "choice variants require `name = \"...\"`")
            })?;
            validate_choice_name(&name)?;
            let value = args.value.as_ref().map(parse_value).transpose()?;
            if value.is_none()
                && variant.ident.unraw().to_string().chars().count() > MAX_CHOICE_STRING_VALUE
            {
                return Err(Error::new(
                    variant.ident.span(),
                    "Discord string choice values must contain at most 100 characters",
                ));
            }

            Ok(VariantChoice {
                variant: variant.ident.clone(),
                name,
                value,
            })
        })
        .collect()
}

fn choice_attribute(attributes: &[Attribute]) -> Result<Option<&Attribute>> {
    let mut found = None;
    for attribute in attributes {
        if !attribute.path().is_ident("choice") {
            continue;
        }
        if found.replace(attribute).is_some() {
            return Err(Error::new_spanned(
                attribute,
                "duplicate `#[choice(...)]` attribute",
            ));
        }
    }
    Ok(found)
}

fn validate_choice_name(name: &LitStr) -> Result<()> {
    let length = name.value().chars().count();
    if !(1..=100).contains(&length) {
        return Err(Error::new(
            name.span(),
            "Discord choice names must contain between 1 and 100 characters",
        ));
    }
    Ok(())
}

fn parse_value(expression: &Expr) -> Result<ParsedValue> {
    let literal = literal(expression).ok_or_else(|| {
        Error::new_spanned(
            expression,
            "choice values require string, integer, or number literals",
        )
    })?;

    match literal {
        Lit::Str(value) => {
            if value.value().chars().count() > MAX_CHOICE_STRING_VALUE {
                return Err(Error::new(
                    value.span(),
                    "Discord string choice values must contain at most 100 characters",
                ));
            }
            Ok(ParsedValue::String(value.clone()))
        }
        Lit::Int(_) => Ok(ParsedValue::Integer(parse_integer_bound(expression)?)),
        Lit::Float(_) => Ok(ParsedValue::Number(parse_number_bound(expression)?)),
        _ => Err(Error::new_spanned(
            expression,
            "choice values require string, integer, or number literals",
        )),
    }
}

fn literal(expression: &Expr) -> Option<&Lit> {
    match expression {
        Expr::Lit(ExprLit { lit, .. }) => Some(lit),
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        }) => match expr.as_ref() {
            Expr::Lit(ExprLit { lit, .. }) => Some(lit),
            _ => None,
        },
        _ => None,
    }
}

fn infer_kind(choices: &[VariantChoice]) -> Result<ChoiceKind> {
    let has_default = choices.iter().any(|choice| choice.value.is_none());
    let has_string = choices
        .iter()
        .any(|choice| matches!(choice.value, Some(ParsedValue::String(_))));
    let has_integer = choices
        .iter()
        .any(|choice| matches!(choice.value, Some(ParsedValue::Integer(_))));
    let has_number = choices
        .iter()
        .any(|choice| matches!(choice.value, Some(ParsedValue::Number(_))));

    if (has_default || has_string) && (has_integer || has_number) {
        return Err(Error::new_spanned(
            &choices[0].variant,
            "command-choice enum values must all use the same scalar kind; omitted values are string choices",
        ));
    }
    if has_default || has_string {
        return Ok(ChoiceKind::String);
    }
    if has_number {
        return Ok(ChoiceKind::Number);
    }
    Ok(ChoiceKind::Integer)
}

fn validate_duplicates(choices: &[VariantChoice], kind: ChoiceKind) -> Result<()> {
    let mut names = HashSet::new();
    let mut values = HashSet::new();

    for choice in choices {
        if !names.insert(choice.name.value()) {
            return Err(Error::new(
                choice.name.span(),
                "duplicate Discord choice name",
            ));
        }

        let value = value_key(choice, kind)?;
        if !values.insert(value) {
            return Err(Error::new(
                choice.variant.span(),
                "duplicate Discord choice value",
            ));
        }
    }
    Ok(())
}

fn value_key(choice: &VariantChoice, kind: ChoiceKind) -> Result<String> {
    Ok(match (kind, choice.value.as_ref()) {
        (ChoiceKind::String, None) => choice.variant.unraw().to_string(),
        (ChoiceKind::String, Some(ParsedValue::String(value))) => value.value(),
        (ChoiceKind::Integer, Some(ParsedValue::Integer(value))) => format!("i:{value}"),
        (ChoiceKind::Number, Some(ParsedValue::Integer(value))) => number_key(*value as f64),
        (ChoiceKind::Number, Some(ParsedValue::Number(value))) => number_key(*value),
        _ => {
            return Err(Error::new(
                choice.variant.span(),
                "command-choice enum values do not match the inferred choice kind",
            ));
        }
    })
}

fn number_key(value: f64) -> String {
    if value == 0.0 {
        "n:0".to_owned()
    } else {
        format!("n:{value}")
    }
}

fn descriptor_tokens(choice: &VariantChoice, kind: ChoiceKind) -> Result<TokenStream> {
    let name = &choice.name;
    Ok(match (kind, choice.value.as_ref()) {
        (ChoiceKind::String, None) => {
            let value = LitStr::new(&choice.variant.unraw().to_string(), choice.variant.span());
            quote! { ::gloam_commands::CommandChoiceDescriptor::string(#name, #value) }
        }
        (ChoiceKind::String, Some(ParsedValue::String(value))) => {
            quote! { ::gloam_commands::CommandChoiceDescriptor::string(#name, #value) }
        }
        (ChoiceKind::Integer, Some(ParsedValue::Integer(value))) => {
            quote! { ::gloam_commands::CommandChoiceDescriptor::integer(#name, #value) }
        }
        (ChoiceKind::Number, Some(ParsedValue::Integer(value))) => {
            let value = *value as f64;
            quote! { ::gloam_commands::CommandChoiceDescriptor::number(#name, #value) }
        }
        (ChoiceKind::Number, Some(ParsedValue::Number(value))) => {
            quote! { ::gloam_commands::CommandChoiceDescriptor::number(#name, #value) }
        }
        _ => {
            return Err(Error::new(
                choice.variant.span(),
                "command-choice enum values do not match the inferred choice kind",
            ));
        }
    })
}

fn extraction_tokens(choices: &[VariantChoice], kind: ChoiceKind) -> TokenStream {
    match kind {
        ChoiceKind::String => {
            let branches = choices.iter().map(|choice| {
                let variant = &choice.variant;
                let value = match choice.value.as_ref() {
                    None => LitStr::new(&variant.unraw().to_string(), variant.span()),
                    Some(ParsedValue::String(value)) => value.clone(),
                    _ => unreachable!("choice kind was validated"),
                };
                quote! { #value => ::std::result::Result::Ok(Self::#variant), }
            });
            quote! {
                let value = <::std::string::String as ::gloam_commands::CommandOption>::extract(
                    options,
                    name,
                )?;
                match value.as_str() {
                    #(#branches)*
                    _ => ::std::result::Result::Err(::gloam_commands::Error::InvalidChoice { name }),
                }
            }
        }
        ChoiceKind::Integer => {
            let branches = choices.iter().map(|choice| {
                let variant = &choice.variant;
                let Some(ParsedValue::Integer(value)) = choice.value.as_ref() else {
                    unreachable!("choice kind was validated");
                };
                quote! { #value => ::std::result::Result::Ok(Self::#variant), }
            });
            quote! {
                let value = <i64 as ::gloam_commands::CommandOption>::extract(options, name)?;
                match value {
                    #(#branches)*
                    _ => ::std::result::Result::Err(::gloam_commands::Error::InvalidChoice { name }),
                }
            }
        }
        ChoiceKind::Number => {
            let branches = choices.iter().map(|choice| {
                let variant = &choice.variant;
                let value = match choice.value.as_ref() {
                    Some(ParsedValue::Integer(value)) => *value as f64,
                    Some(ParsedValue::Number(value)) => *value,
                    _ => unreachable!("choice kind was validated"),
                };
                quote! {
                    if value == #value {
                        return ::std::result::Result::Ok(Self::#variant);
                    }
                }
            });
            quote! {
                let value = <f64 as ::gloam_commands::CommandOption>::extract(options, name)?;
                #(#branches)*
                ::std::result::Result::Err(::gloam_commands::Error::InvalidChoice { name })
            }
        }
    }
}

fn option_kind_tokens(kind: ChoiceKind) -> TokenStream {
    match kind {
        ChoiceKind::String => quote! {
            <::std::string::String as ::gloam_commands::CommandOption>::KIND
        },
        ChoiceKind::Integer => quote! {
            <i64 as ::gloam_commands::CommandOption>::KIND
        },
        ChoiceKind::Number => quote! {
            <f64 as ::gloam_commands::CommandOption>::KIND
        },
    }
}
