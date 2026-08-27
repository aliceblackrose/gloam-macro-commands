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
            "`#[autocomplete]` does not accept arguments",
        ));
    }

    let function = parse2::<ItemFn>(item)?;
    validate_signature(&function)?;

    let function_name = function.sig.ident.clone();
    let context_type = autocomplete_context_type(&function)?.clone();
    let visibility = function.vis.clone();
    let helper_suffix = function_name.unraw().to_string();
    let handler_name = format_ident!(
        "__gloam_autocomplete_{helper_suffix}",
        span = function_name.span()
    );

    Ok(quote! {
        #function

        #[doc(hidden)]
        #visibility fn #handler_name(ctx: #context_type) -> ::gloam_commands::AutocompleteFuture {
            ::std::boxed::Box::pin(#function_name(ctx))
        }
    })
}

fn validate_signature(function: &ItemFn) -> Result<()> {
    let signature = &function.sig;

    if signature.asyncness.is_none() {
        return Err(Error::new_spanned(
            signature.fn_token,
            "autocomplete handlers must be declared with `async fn`",
        ));
    }
    if signature.constness.is_some() {
        return Err(Error::new_spanned(
            signature.constness,
            "autocomplete handlers cannot be `const`",
        ));
    }
    if signature.unsafety.is_some() {
        return Err(Error::new_spanned(
            signature.unsafety,
            "autocomplete handlers cannot be `unsafe`",
        ));
    }
    if signature.abi.is_some() {
        return Err(Error::new_spanned(
            &signature.abi,
            "autocomplete handlers cannot declare an explicit ABI",
        ));
    }
    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &signature.generics,
            "generic autocomplete handlers are not supported",
        ));
    }
    if signature.variadic.is_some() {
        return Err(Error::new_spanned(
            &signature.variadic,
            "variadic autocomplete handlers are not supported",
        ));
    }
    if signature.inputs.len() != 1 {
        return Err(Error::new_spanned(
            &signature.inputs,
            "autocomplete handlers require exactly one `AutocompleteContext<D>` parameter",
        ));
    }

    let _ = autocomplete_context_type(function)?;
    validate_return_type(&signature.output)
}

fn autocomplete_context_type(function: &ItemFn) -> Result<&Type> {
    let Some(argument) = function.sig.inputs.first() else {
        return Err(Error::new(
            function.sig.ident.span(),
            "autocomplete handlers require an `AutocompleteContext<D>` parameter",
        ));
    };
    let FnArg::Typed(argument) = argument else {
        return Err(Error::new_spanned(
            argument,
            "autocomplete handlers must be free functions whose parameter is `AutocompleteContext<D>`",
        ));
    };
    let Type::Path(context) = argument.ty.as_ref() else {
        return Err(Error::new_spanned(
            &argument.ty,
            "autocomplete context parameter must have type `AutocompleteContext<D>`",
        ));
    };
    let Some(segment) = context.path.segments.last() else {
        return Err(Error::new_spanned(
            &argument.ty,
            "autocomplete context parameter must have type `AutocompleteContext<D>`",
        ));
    };
    if segment.ident != "AutocompleteContext" {
        return Err(Error::new_spanned(
            &argument.ty,
            "autocomplete context parameter must have type `AutocompleteContext<D>`",
        ));
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(Error::new_spanned(
            &argument.ty,
            "`AutocompleteContext` requires exactly one application-state type parameter",
        ));
    };
    if arguments.args.len() != 1
        || !matches!(arguments.args.first(), Some(GenericArgument::Type(_)))
    {
        return Err(Error::new_spanned(
            arguments,
            "`AutocompleteContext` requires exactly one application-state type parameter",
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
    let Some(GenericArgument::Type(Type::Path(vector))) = arguments.args.first() else {
        return Err(invalid_return_type(output));
    };
    let Some(vector_segment) = vector.path.segments.last() else {
        return Err(invalid_return_type(output));
    };
    if vector_segment.ident != "Vec" {
        return Err(invalid_return_type(output));
    }
    let PathArguments::AngleBracketed(vector_arguments) = &vector_segment.arguments else {
        return Err(invalid_return_type(output));
    };
    if vector_arguments.args.len() != 1 {
        return Err(invalid_return_type(output));
    }
    let Some(GenericArgument::Type(Type::Path(choice))) = vector_arguments.args.first() else {
        return Err(invalid_return_type(output));
    };
    let Some(choice_segment) = choice.path.segments.last() else {
        return Err(invalid_return_type(output));
    };
    if choice_segment.ident != "AutocompleteChoice"
        || !matches!(choice_segment.arguments, PathArguments::None)
    {
        return Err(invalid_return_type(output));
    }

    Ok(())
}

fn invalid_return_type(tokens: impl quote::ToTokens) -> Error {
    Error::new_spanned(
        tokens,
        "autocomplete handlers must return `Result<Vec<AutocompleteChoice>>`",
    )
}
