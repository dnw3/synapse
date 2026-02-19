use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Parser, parse2, punctuated::Punctuated, Expr, ItemFn, Lit, Meta, Token};

// ---------------------------------------------------------------------------
// Attribute-level config: #[entrypoint(name = "...", checkpointer = "...")]
// ---------------------------------------------------------------------------

struct EntrypointAttr {
    name: Option<String>,
    checkpointer: Option<String>,
}

fn parse_entrypoint_attr(attr: TokenStream) -> syn::Result<EntrypointAttr> {
    let mut name = None;
    let mut checkpointer = None;

    if attr.is_empty() {
        return Ok(EntrypointAttr { name, checkpointer });
    }

    let meta_list: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated.parse2(attr)?;

    for meta in meta_list {
        if let Meta::NameValue(nv) = meta {
            let key = nv
                .path
                .get_ident()
                .map(|i| i.to_string())
                .unwrap_or_default();
            if let Expr::Lit(expr_lit) = &nv.value {
                if let Lit::Str(lit_str) = &expr_lit.lit {
                    match key.as_str() {
                        "name" => name = Some(lit_str.value()),
                        "checkpointer" => checkpointer = Some(lit_str.value()),
                        _ => {
                            return Err(syn::Error::new_spanned(
                                &nv.path,
                                format!("unknown entrypoint attribute: `{}`", key),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(EntrypointAttr { name, checkpointer })
}

// ---------------------------------------------------------------------------
// Main expansion
// ---------------------------------------------------------------------------

pub fn expand_entrypoint(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let ep_attr = parse_entrypoint_attr(attr)?;
    let func: ItemFn = parse2(item)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;

    // Name defaults to the function name
    let ep_name_str = ep_attr.name.unwrap_or_else(|| fn_name.to_string());

    // Checkpointer: Option<&'static str>
    let checkpointer_expr = match &ep_attr.checkpointer {
        Some(cp) => quote! { ::std::option::Option::Some(#cp) },
        None => quote! { ::std::option::Option::None },
    };

    // Validate the function signature:
    // - must be async
    // - must take a single Value parameter
    // - must return Result<Value, SynapticError>
    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            func.sig.fn_token,
            "#[entrypoint] function must be async",
        ));
    }

    let fn_body = &func.block;

    // Extract the parameter ident and type for the closure signature
    let params: Vec<_> = func
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pt) = arg {
                if let syn::Pat::Ident(pi) = &*pt.pat {
                    return Some((pi.ident.clone(), (*pt.ty).clone()));
                }
            }
            None
        })
        .collect();

    if params.len() != 1 {
        return Err(syn::Error::new_spanned(
            &func.sig.inputs,
            "#[entrypoint] function must accept exactly one parameter (serde_json::Value)",
        ));
    }

    let (param_ident, param_ty) = &params[0];

    Ok(quote! {
        #vis fn #fn_name() -> ::synaptic_core::Entrypoint {
            ::synaptic_core::Entrypoint {
                config: ::synaptic_core::EntrypointConfig {
                    name: #ep_name_str,
                    checkpointer: #checkpointer_expr,
                },
                invoke_fn: ::std::boxed::Box::new(|#param_ident: #param_ty| {
                    ::std::boxed::Box::pin(async move #fn_body)
                }),
            }
        }
    })
}
