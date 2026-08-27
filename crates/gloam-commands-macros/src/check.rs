use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, GenericArgument, ItemFn, PathArguments, Result, ReturnType, Type, ext::IdentExt,
    parse2,
};

pub(crate) fn expand(attribute: TokenStream, item: TokenStream) -> Result<TokenStream> {
    if !attribute.is_empty() {
        return Err(Error::new_spanned(
            attribute,
            "`#[check]` does not accept arguments",
        ));
    }

    let function = parse2::<ItemFn>(item)?;
    validate_signature(&function)?;

    let function_name = function.sig.ident.clone();
    let context_type = context_type(&function)?.clone();
    let visibility = function.vis.clone();
    let helper_suffix = function_name.unraw().to_string();
    let handler_name = format_ident!("__gloam_check_{helper_suffix}", span = function_name.span());

    Ok(quote! {
        #function

        #[doc(hidden)]
        #visibility fn #handler_name(ctx: #context_type) -> ::gloam_commands::CheckFuture {
            ::std::boxed::Box::pin(#function_name(ctx))
        }
    })
}

fn validate_signature(function: &ItemFn) -> Result<()> {
    let signature = &function.sig;

    if signature.asyncness.is_none() {
        return Err(Error::new_spanned(
            signature.fn_token,
            "command checks must be declared with `async fn`",
        ));
    }
    if signature.constness.is_some() {
        return Err(Error::new_spanned(
            signature.constness,
            "command checks cannot be `const`",
        ));
    }
    if signature.unsafety.is_some() {
        return Err(Error::new_spanned(
            signature.unsafety,
            "command checks cannot be `unsafe`",
        ));
    }
    if signature.abi.is_some() {
        return Err(Error::new_spanned(
            &signature.abi,
            "command checks cannot declare an explicit ABI",
        ));
    }
    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &signature.generics,
            "generic command checks are not supported",
        ));
    }
    if signature.variadic.is_some() {
        return Err(Error::new_spanned(
            &signature.variadic,
            "variadic command checks are not supported",
        ));
    }
    if signature.inputs.len() != 1 {
        return Err(Error::new_spanned(
            &signature.inputs,
            "command checks require exactly one `Context<D>` parameter",
        ));
    }

    let _ = context_type(function)?;
    validate_return_type(&signature.output)
}

fn context_type(function: &ItemFn) -> Result<&Type> {
    let Some(argument) = function.sig.inputs.first() else {
        return Err(Error::new(
            function.sig.ident.span(),
            "command checks require a `Context<D>` parameter",
        ));
    };
    let FnArg::Typed(argument) = argument else {
        return Err(Error::new_spanned(
            argument,
            "command checks must be free functions whose parameter is `Context<D>`",
        ));
    };
    let Type::Path(context) = argument.ty.as_ref() else {
        return Err(Error::new_spanned(
            &argument.ty,
            "command check context parameter must have type `Context<D>`",
        ));
    };
    let Some(segment) = context.path.segments.last() else {
        return Err(Error::new_spanned(
            &argument.ty,
            "command check context parameter must have type `Context<D>`",
        ));
    };
    if segment.ident != "Context" {
        return Err(Error::new_spanned(
            &argument.ty,
            "command check context parameter must have type `Context<D>`",
        ));
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(Error::new_spanned(
            &argument.ty,
            "`Context` requires exactly one application-state type parameter",
        ));
    };
    if arguments.args.len() != 1
        || !matches!(arguments.args.first(), Some(GenericArgument::Type(_)))
    {
        return Err(Error::new_spanned(
            arguments,
            "`Context` requires exactly one application-state type parameter",
        ));
    }

    Ok(argument.ty.as_ref())
}

fn validate_return_type(output: &ReturnType) -> Result<()> {
    let ReturnType::Type(_, output) = output else {
        return Err(invalid_return_type(output));
    };
    let Type::Path(result) = output.as_ref() else {
        return Err(invalid_return_type(output));
    };
    let Some(segment) = result.path.segments.last() else {
        return Err(invalid_return_type(output));
    };
    if segment.ident != "Result" {
        return Err(invalid_return_type(output));
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(invalid_return_type(output));
    };
    if arguments.args.len() != 1 {
        return Err(invalid_return_type(output));
    }
    let Some(GenericArgument::Type(Type::Path(value))) = arguments.args.first() else {
        return Err(invalid_return_type(output));
    };
    let Some(value_segment) = value.path.segments.last() else {
        return Err(invalid_return_type(output));
    };
    if value_segment.ident != "bool" || !matches!(value_segment.arguments, PathArguments::None) {
        return Err(invalid_return_type(output));
    }

    Ok(())
}

fn invalid_return_type(tokens: impl quote::ToTokens) -> Error {
    Error::new_spanned(tokens, "command checks must return `Result<bool>`")
}
