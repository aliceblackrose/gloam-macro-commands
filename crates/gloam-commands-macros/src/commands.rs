use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, Path, PathArguments, Result, Token,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse2,
    punctuated::Punctuated,
};

struct CommandPaths {
    paths: Punctuated<Path, Token![,]>,
}

impl Parse for CommandPaths {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(Self {
            paths: Punctuated::parse_terminated(input)?,
        })
    }
}

pub(crate) fn expand(input: TokenStream) -> Result<TokenStream> {
    let commands = parse2::<CommandPaths>(input)?;
    let factories = commands
        .paths
        .iter()
        .map(factory_path)
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        ::std::vec![#(#factories()),*]
    })
}

fn factory_path(command: &Path) -> Result<Path> {
    let mut factory = command.clone();
    let Some(segment) = factory.segments.last_mut() else {
        return Err(Error::new_spanned(command, "expected a slash-command path"));
    };

    if !matches!(segment.arguments, PathArguments::None) {
        return Err(Error::new_spanned(
            &segment.arguments,
            "slash-command paths cannot contain generic arguments",
        ));
    }

    let suffix = segment.ident.unraw().to_string();
    segment.ident = format_ident!("__gloam_command_{suffix}", span = segment.ident.span());
    Ok(factory)
}
