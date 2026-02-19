use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::Parser, parse2, punctuated::Punctuated, Expr, ItemFn, Lit, Meta, Token};

// ---------------------------------------------------------------------------
// Attribute-level config: #[task(name = "...")]
// ---------------------------------------------------------------------------

struct TaskAttr {
    name: Option<String>,
}

fn parse_task_attr(attr: TokenStream) -> syn::Result<TaskAttr> {
    let mut name = None;

    if attr.is_empty() {
        return Ok(TaskAttr { name });
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
                        _ => {
                            return Err(syn::Error::new_spanned(
                                &nv.path,
                                format!("unknown task attribute: `{}`", key),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(TaskAttr { name })
}

// ---------------------------------------------------------------------------
// Main expansion
// ---------------------------------------------------------------------------

pub fn expand_task(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let task_attr = parse_task_attr(attr)?;
    let func: ItemFn = parse2(item)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    // The task name for tracing; defaults to the function name
    let task_name_str = task_attr.name.unwrap_or_else(|| fn_name.to_string());

    // Validate async
    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            func.sig.fn_token,
            "#[task] function must be async",
        ));
    }

    // Rename the original function to `{name}_impl`
    let mut impl_func = func.clone();
    impl_func.sig.ident = impl_fn_name.clone();
    // Strip outer-level attributes (doc comments etc.) from the impl — they
    // stay on the public wrapper instead.
    impl_func.attrs.retain(|a| !a.path().is_ident("doc"));

    // Extract parameter idents and types for forwarding
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

    let fn_params = &func.sig.inputs;
    let fn_ret = &func.sig.output;
    let fn_attrs: Vec<_> = func.attrs.iter().collect();

    // The wrapper function has a `#[allow(unused_variables)]` for the name
    // constant, since it is currently only a marker.  If the tracing feature
    // or a runtime callback system is wired up later, it can use `__task_name`.
    Ok(quote! {
        #impl_func

        #(#fn_attrs)*
        #vis async fn #fn_name(#fn_params) #fn_ret {
            #[allow(dead_code)]
            const __TASK_NAME: &str = #task_name_str;
            #impl_fn_name(#(#param_idents),*).await
        }
    })
}
