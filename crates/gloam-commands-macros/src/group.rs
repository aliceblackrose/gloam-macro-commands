use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Error, Ident, Item, ItemFn, ItemMod, LitStr, Result, Token, Type,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse2,
};

use crate::command;

struct GroupArgs {
    name: Option<LitStr>,
    description: Option<LitStr>,
}

impl Parse for GroupArgs {
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
                    "unsupported group argument; expected `name` or `description`",
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
    let args = parse2::<GroupArgs>(attribute)?;
    let mut module = parse2::<ItemMod>(item)?;
    let module_ident = module.ident.clone();
    let group_name = args
        .name
        .unwrap_or_else(|| LitStr::new(&module_ident.unraw().to_string(), module_ident.span()));
    let description = args.description.ok_or_else(|| {
        Error::new(
            module_ident.span(),
            "slash-command groups require `description = \"...\"`",
        )
    })?;
    command::validate_name(&group_name)?;
    command::validate_description(&description)?;

    let visibility = module.vis.clone();
    let Some((_, items)) = module.content.as_mut() else {
        return Err(Error::new_spanned(
            &module,
            "`#[group]` requires an inline module body",
        ));
    };
    validate_native_hierarchy(items)?;

    let child_factories = child_factories(items)?;
    if child_factories.is_empty() {
        return Err(Error::new(
            module_ident.span(),
            "slash-command groups require at least one direct `#[command]` or `#[group]` child",
        ));
    }
    let state_type = find_state_type(items)?.ok_or_else(|| {
        Error::new(
            module_ident.span(),
            "slash-command groups must contain at least one `#[command]` leaf",
        )
    })?;

    let suffix = module_ident.unraw().to_string();
    let inner_factory = format_ident!("__gloam_group_factory_{suffix}", span = module_ident.span());
    let outer_factory = format_ident!("__gloam_command_{suffix}", span = module_ident.span());
    let factory: ItemFn = parse2(quote! {
        #[doc(hidden)]
        pub(super) fn #inner_factory() -> ::gloam_commands::SlashCommand<#state_type> {
            static DESCRIPTOR: ::gloam_commands::CommandDescriptor =
                ::gloam_commands::CommandDescriptor::new(#group_name, #description);

            ::gloam_commands::SlashCommand::group(
                &DESCRIPTOR,
                ::std::vec![#(#child_factories),*],
            )
        }
    })?;
    items.push(Item::Fn(factory));

    Ok(quote! {
        #module

        #[doc(hidden)]
        #visibility fn #outer_factory() -> ::gloam_commands::SlashCommand<#state_type> {
            #module_ident::#inner_factory()
        }
    })
}

fn child_factories(items: &[Item]) -> Result<Vec<TokenStream>> {
    let mut factories = Vec::new();

    for item in items {
        match item {
            Item::Fn(function) if has_attribute(&function.attrs, "command") => {
                let suffix = function.sig.ident.unraw().to_string();
                let factory = format_ident!(
                    "__gloam_command_{suffix}",
                    span = function.sig.ident.span()
                );
                factories.push(quote! { #factory() });
            }
            Item::Mod(module) if has_attribute(&module.attrs, "group") => {
                let suffix = module.ident.unraw().to_string();
                let factory = format_ident!("__gloam_command_{suffix}", span = module.ident.span());
                factories.push(quote! { #factory() });
            }
            _ => {}
        }
    }

    Ok(factories)
}

fn find_state_type(items: &[Item]) -> Result<Option<Type>> {
    for item in items {
        match item {
            Item::Fn(function) if has_attribute(&function.attrs, "command") => {
                let context = command::context_type(function)?;
                return command::state_type(context).cloned().map(Some);
            }
            Item::Mod(module) if has_attribute(&module.attrs, "group") => {
                let Some((_, nested)) = module.content.as_ref() else {
                    return Err(Error::new_spanned(
                        module,
                        "`#[group]` requires an inline module body",
                    ));
                };
                if let Some(state) = find_state_type(nested)? {
                    return Ok(Some(state));
                }
            }
            _ => {}
        }
    }

    Ok(None)
}

fn validate_native_hierarchy(items: &[Item]) -> Result<()> {
    for item in items {
        let Item::Mod(group) = item else {
            continue;
        };
        if !has_attribute(&group.attrs, "group") {
            continue;
        }
        let Some((_, children)) = group.content.as_ref() else {
            return Err(Error::new_spanned(
                group,
                "`#[group]` requires an inline module body",
            ));
        };
        if let Some(nested) = children.iter().find_map(|item| match item {
            Item::Mod(module) if has_attribute(&module.attrs, "group") => Some(module),
            _ => None,
        }) {
            return Err(Error::new_spanned(
                nested,
                "Discord slash commands support at most `command -> subcommand group -> subcommand`",
            ));
        }
    }

    Ok(())
}

fn has_attribute(attributes: &[Attribute], name: &str) -> bool {
    attributes.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name)
    })
}
