use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Parser, parse2, punctuated::Punctuated, Expr, ItemFn, Lit, Meta, Token};

// ---------------------------------------------------------------------------
// Attribute-level config: #[traceable(name = "...", skip = "a,b")]
// ---------------------------------------------------------------------------

struct TraceableAttr {
    name: Option<String>,
    skip: Vec<String>,
}

fn parse_traceable_attr(attr: TokenStream) -> syn::Result<TraceableAttr> {
    let mut name = None;
    let mut skip = Vec::new();

    if attr.is_empty() {
        return Ok(TraceableAttr { name, skip });
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
                        "skip" => {
                            skip = lit_str
                                .value()
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        _ => {
                            return Err(syn::Error::new_spanned(
                                &nv.path,
                                format!("unknown traceable attribute: `{}`", key),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(TraceableAttr { name, skip })
}

// ---------------------------------------------------------------------------
// Main expansion
// ---------------------------------------------------------------------------

/// Expand `#[traceable]` on an async or sync function.
///
/// Wraps the function body with `tracing::info_span!` instrumentation.
///
/// Given:
/// ```ignore
/// #[traceable]
/// async fn my_func(a: String, b: i32) -> Result<String, Error> { ... }
/// ```
///
/// Produces:
/// ```ignore
/// async fn my_func(a: String, b: i32) -> Result<String, Error> {
///     let __span = ::tracing::info_span!("my_func", a = %a, b = %b);
///     let __guard = __span.enter();
///     drop(__guard); // for async — use Instrument instead
///     async move { ... }.instrument(__span).await
/// }
/// ```
pub fn expand_traceable(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let trace_attr = parse_traceable_attr(attr)?;
    let func: ItemFn = parse2(item)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let fn_attrs: Vec<_> = func.attrs.iter().collect();
    let fn_params = &func.sig.inputs;
    let fn_ret = &func.sig.output;
    let fn_body = &func.block;
    let fn_generics = &func.sig.generics;
    let fn_where = &func.sig.generics.where_clause;

    let span_name = trace_attr.name.unwrap_or_else(|| fn_name.to_string());

    // Collect parameter names for span fields, skipping those in the skip list
    let param_idents: Vec<_> = func
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pt) = arg {
                if let syn::Pat::Ident(pi) = &*pt.pat {
                    let name = pi.ident.to_string();
                    if !trace_attr.skip.contains(&name) {
                        return Some(pi.ident.clone());
                    }
                }
            }
            None
        })
        .collect();

    let field_exprs: Vec<TokenStream> = param_idents
        .iter()
        .map(|ident| {
            quote! { #ident = ::tracing::field::debug(&#ident) }
        })
        .collect();

    let is_async = func.sig.asyncness.is_some();

    if is_async {
        // For async functions, use Instrument trait
        Ok(quote! {
            #(#fn_attrs)*
            #vis async fn #fn_name #fn_generics (#fn_params) #fn_ret #fn_where {
                use ::tracing::Instrument;
                let __span = ::tracing::info_span!(#span_name, #(#field_exprs),*);
                async move #fn_body.instrument(__span).await
            }
        })
    } else {
        // For sync functions, use span guard
        Ok(quote! {
            #(#fn_attrs)*
            #vis fn #fn_name #fn_generics (#fn_params) #fn_ret #fn_where {
                let __span = ::tracing::info_span!(#span_name, #(#field_exprs),*);
                let __enter = __span.enter();
                #fn_body
            }
        })
    }
}
