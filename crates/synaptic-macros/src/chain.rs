use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, GenericArgument, ItemFn, PathArguments, ReturnType, Type};

use crate::paths;

/// Check if a type is `Value` or `serde_json::Value`.
fn is_value_type(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident == "Value";
        }
    }
    false
}

/// Extract the `T` from `Result<T, E>` in the function's return type.
fn extract_result_ok_type(ret: &ReturnType) -> Option<&Type> {
    if let ReturnType::Type(_, ty) = ret {
        if let Type::Path(tp) = ty.as_ref() {
            if let Some(seg) = tp.path.segments.last() {
                if seg.ident == "Result" {
                    if let PathArguments::AngleBracketed(args) = &seg.arguments {
                        if let Some(GenericArgument::Type(ok_ty)) = args.args.first() {
                            return Some(ok_ty);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Expand `#[chain]` on an async function.
///
/// Given:
/// ```ignore
/// #[chain]
/// async fn my_chain(input: Value) -> Result<Value, SynapticError> { ... }
/// ```
///
/// Produces:
/// ```ignore
/// async fn my_chain_impl(input: Value) -> Result<Value, SynapticError> { ... }
///
/// pub fn my_chain() -> BoxRunnable<Value, Value> {
///     RunnableLambda::new(|input: Value| async move {
///         my_chain_impl(input).await
///     }).boxed()
/// }
/// ```
///
/// When the return type is `Result<T, _>` where T is not `Value`, the macro
/// generates `BoxRunnable<InputType, T>` without serialization:
/// ```ignore
/// #[chain]
/// async fn to_upper(s: String) -> Result<String, SynapticError> { ... }
/// // Generates: fn to_upper() -> BoxRunnable<String, String>
/// ```
pub fn expand_chain(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[chain] does not accept arguments",
        ));
    }

    let func: ItemFn = parse2(item)?;
    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    // Rename the original function
    let mut impl_func = func.clone();
    impl_func.sig.ident = impl_fn_name.clone();
    // Remove #[chain] from attrs if present (shouldn't be, but be safe)
    impl_func.attrs.retain(|a| !a.path().is_ident("chain"));

    // Extract parameter idents for forwarding
    let param_idents: Vec<_> = func
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pt) = arg {
                if let syn::Pat::Ident(pi) = &*pt.pat {
                    return Some(pi.ident.clone());
                }
            }
            None
        })
        .collect();

    // Extract parameter types for the closure signature
    let param_types: Vec<_> = func
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pt) = arg {
                Some((*pt.ty).clone())
            } else {
                None
            }
        })
        .collect();

    // Determine if the output type needs serialization to Value
    let ok_type = extract_result_ok_type(&func.sig.output);
    let needs_serialize = ok_type.is_none_or(is_value_type);

    let core_crate = paths::core_path();
    let runnables_crate = paths::runnables_path();

    // For a single-parameter function, generate a simple RunnableLambda
    if param_idents.len() == 1 {
        let p_ident = &param_idents[0];
        let p_type = &param_types[0];

        if needs_serialize {
            Ok(quote! {
                #impl_func

                #vis fn #fn_name() -> #runnables_crate::BoxRunnable<#p_type, ::serde_json::Value> {
                    #runnables_crate::RunnableLambda::new(
                        |#p_ident: #p_type| async move {
                            let __result = #impl_fn_name(#p_ident).await?;
                            ::serde_json::to_value(__result)
                                .map_err(|__e| #core_crate::SynapticError::Parsing(
                                    format!("chain serialization error: {}", __e)
                                ))
                        }
                    ).boxed()
                }
            })
        } else {
            let out_type = ok_type.unwrap();
            Ok(quote! {
                #impl_func

                #vis fn #fn_name() -> #runnables_crate::BoxRunnable<#p_type, #out_type> {
                    #runnables_crate::RunnableLambda::new(
                        |#p_ident: #p_type| async move {
                            #impl_fn_name(#p_ident).await
                        }
                    ).boxed()
                }
            })
        }
    } else {
        // Multi-parameter: wrap as Value -> Output
        if needs_serialize {
            Ok(quote! {
                #impl_func

                #vis fn #fn_name() -> #runnables_crate::BoxRunnable<::serde_json::Value, ::serde_json::Value> {
                    #runnables_crate::RunnableLambda::new(
                        |__input: ::serde_json::Value| async move {
                            let __result = #impl_fn_name(__input).await?;
                            ::serde_json::to_value(__result)
                                .map_err(|__e| #core_crate::SynapticError::Parsing(
                                    format!("chain serialization error: {}", __e)
                                ))
                        }
                    ).boxed()
                }
            })
        } else {
            let out_type = ok_type.unwrap();
            Ok(quote! {
                #impl_func

                #vis fn #fn_name() -> #runnables_crate::BoxRunnable<::serde_json::Value, #out_type> {
                    #runnables_crate::RunnableLambda::new(
                        |__input: ::serde_json::Value| async move {
                            #impl_fn_name(__input).await
                        }
                    ).boxed()
                }
            })
        }
    }
}
