use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, FnArg, ItemFn, Pat, Type};

// ---------------------------------------------------------------------------
// Helper: PascalCase
// ---------------------------------------------------------------------------

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let mut result = c.to_uppercase().to_string();
                    result.extend(chars);
                    result
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// #[field] support for middleware macros
// ---------------------------------------------------------------------------

struct FieldParam {
    ident: syn::Ident,
    ty: Type,
}

/// Parse `#[field]` attributes from function parameters.
/// Returns field params and a cleaned function with `#[field]` attrs stripped.
fn extract_field_params(func: &ItemFn) -> syn::Result<(Vec<FieldParam>, ItemFn)> {
    let mut fields = Vec::new();
    let mut clean = func.clone();

    for arg in &mut clean.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            let has_field = pt.attrs.iter().any(|a| a.path().is_ident("field"));
            if has_field {
                let ident = if let Pat::Ident(pi) = &*pt.pat {
                    pi.ident.clone()
                } else {
                    return Err(syn::Error::new_spanned(
                        &pt.pat,
                        "expected a simple identifier for #[field] parameter",
                    ));
                };
                fields.push(FieldParam {
                    ident,
                    ty: (*pt.ty).clone(),
                });
                pt.attrs.retain(|a| !a.path().is_ident("field"));
            }
        }
    }

    Ok((fields, clean))
}

/// Generate struct definition (unit or with fields).
fn gen_middleware_struct(
    vis: &syn::Visibility,
    struct_name: &syn::Ident,
    fields: &[FieldParam],
) -> TokenStream {
    if fields.is_empty() {
        quote! { #vis struct #struct_name; }
    } else {
        let field_defs: Vec<TokenStream> = fields
            .iter()
            .map(|f| {
                let ident = &f.ident;
                let ty = &f.ty;
                quote! { #ident: #ty }
            })
            .collect();
        quote! { #vis struct #struct_name { #(#field_defs),* } }
    }
}

/// Generate factory function.
fn gen_middleware_factory(
    vis: &syn::Visibility,
    fn_name: &syn::Ident,
    struct_name: &syn::Ident,
    fields: &[FieldParam],
) -> TokenStream {
    if fields.is_empty() {
        quote! {
            #vis fn #fn_name() -> ::std::sync::Arc<dyn ::synaptic_middleware::AgentMiddleware> {
                ::std::sync::Arc::new(#struct_name)
            }
        }
    } else {
        let params: Vec<TokenStream> = fields
            .iter()
            .map(|f| {
                let ident = &f.ident;
                let ty = &f.ty;
                quote! { #ident: #ty }
            })
            .collect();
        let inits: Vec<&syn::Ident> = fields.iter().map(|f| &f.ident).collect();
        quote! {
            #vis fn #fn_name(#(#params),*) -> ::std::sync::Arc<dyn ::synaptic_middleware::AgentMiddleware> {
                ::std::sync::Arc::new(#struct_name { #(#inits),* })
            }
        }
    }
}

/// Generate `let x = self.x.clone();` statements for field params.
fn gen_field_clones(fields: &[FieldParam]) -> Vec<TokenStream> {
    fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            quote! { let #ident = self.#ident.clone(); }
        })
        .collect()
}

/// Get field idents for prepending to impl_fn call arguments.
fn field_idents(fields: &[FieldParam]) -> Vec<&syn::Ident> {
    fields.iter().map(|f| &f.ident).collect()
}

// ---------------------------------------------------------------------------
// #[before_agent]
// ---------------------------------------------------------------------------

/// Expand `#[before_agent]` on an async function.
///
/// Supports `#[field]` parameters for stateful middleware.
pub fn expand_before_agent(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[before_agent] does not accept arguments",
        ));
    }

    let func: ItemFn = parse2(item)?;
    let (fields, clean_func) = extract_field_params(&func)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let struct_name = format_ident!("{}Middleware", to_pascal_case(&fn_name.to_string()));
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    let mut impl_func = clean_func;
    impl_func.sig.ident = impl_fn_name.clone();
    impl_func
        .attrs
        .retain(|a| !a.path().is_ident("before_agent"));

    let struct_def = gen_middleware_struct(vis, &struct_name, &fields);
    let factory = gen_middleware_factory(vis, fn_name, &struct_name, &fields);
    let field_clones = gen_field_clones(&fields);
    let fidents = field_idents(&fields);

    Ok(quote! {
        #impl_func

        #struct_def

        #[::async_trait::async_trait]
        impl ::synaptic_middleware::AgentMiddleware for #struct_name {
            async fn before_agent(
                &self,
                messages: &mut Vec<::synaptic_core::Message>,
            ) -> Result<(), ::synaptic_core::SynapticError> {
                #(#field_clones)*
                #impl_fn_name(#(#fidents,)* messages).await
            }
        }

        #factory
    })
}

// ---------------------------------------------------------------------------
// #[before_model]
// ---------------------------------------------------------------------------

/// Expand `#[before_model]` on an async function.
///
/// Supports `#[field]` parameters for stateful middleware.
pub fn expand_before_model(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[before_model] does not accept arguments",
        ));
    }

    let func: ItemFn = parse2(item)?;
    let (fields, clean_func) = extract_field_params(&func)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let struct_name = format_ident!("{}Middleware", to_pascal_case(&fn_name.to_string()));
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    let mut impl_func = clean_func;
    impl_func.sig.ident = impl_fn_name.clone();
    impl_func
        .attrs
        .retain(|a| !a.path().is_ident("before_model"));

    let struct_def = gen_middleware_struct(vis, &struct_name, &fields);
    let factory = gen_middleware_factory(vis, fn_name, &struct_name, &fields);
    let field_clones = gen_field_clones(&fields);
    let fidents = field_idents(&fields);

    Ok(quote! {
        #impl_func

        #struct_def

        #[::async_trait::async_trait]
        impl ::synaptic_middleware::AgentMiddleware for #struct_name {
            async fn before_model(
                &self,
                request: &mut ::synaptic_middleware::ModelRequest,
            ) -> Result<(), ::synaptic_core::SynapticError> {
                #(#field_clones)*
                #impl_fn_name(#(#fidents,)* request).await
            }
        }

        #factory
    })
}

// ---------------------------------------------------------------------------
// #[after_model]
// ---------------------------------------------------------------------------

/// Expand `#[after_model]` on an async function.
///
/// Supports `#[field]` parameters for stateful middleware.
pub fn expand_after_model(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[after_model] does not accept arguments",
        ));
    }

    let func: ItemFn = parse2(item)?;
    let (fields, clean_func) = extract_field_params(&func)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let struct_name = format_ident!("{}Middleware", to_pascal_case(&fn_name.to_string()));
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    let mut impl_func = clean_func;
    impl_func.sig.ident = impl_fn_name.clone();
    impl_func
        .attrs
        .retain(|a| !a.path().is_ident("after_model"));

    let struct_def = gen_middleware_struct(vis, &struct_name, &fields);
    let factory = gen_middleware_factory(vis, fn_name, &struct_name, &fields);
    let field_clones = gen_field_clones(&fields);
    let fidents = field_idents(&fields);

    Ok(quote! {
        #impl_func

        #struct_def

        #[::async_trait::async_trait]
        impl ::synaptic_middleware::AgentMiddleware for #struct_name {
            async fn after_model(
                &self,
                request: &::synaptic_middleware::ModelRequest,
                response: &mut ::synaptic_middleware::ModelResponse,
            ) -> Result<(), ::synaptic_core::SynapticError> {
                #(#field_clones)*
                #impl_fn_name(#(#fidents,)* request, response).await
            }
        }

        #factory
    })
}

// ---------------------------------------------------------------------------
// #[after_agent]
// ---------------------------------------------------------------------------

/// Expand `#[after_agent]` on an async function.
///
/// Supports `#[field]` parameters for stateful middleware.
pub fn expand_after_agent(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[after_agent] does not accept arguments",
        ));
    }

    let func: ItemFn = parse2(item)?;
    let (fields, clean_func) = extract_field_params(&func)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let struct_name = format_ident!("{}Middleware", to_pascal_case(&fn_name.to_string()));
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    let mut impl_func = clean_func;
    impl_func.sig.ident = impl_fn_name.clone();
    impl_func
        .attrs
        .retain(|a| !a.path().is_ident("after_agent"));

    let struct_def = gen_middleware_struct(vis, &struct_name, &fields);
    let factory = gen_middleware_factory(vis, fn_name, &struct_name, &fields);
    let field_clones = gen_field_clones(&fields);
    let fidents = field_idents(&fields);

    Ok(quote! {
        #impl_func

        #struct_def

        #[::async_trait::async_trait]
        impl ::synaptic_middleware::AgentMiddleware for #struct_name {
            async fn after_agent(
                &self,
                messages: &mut Vec<::synaptic_core::Message>,
            ) -> Result<(), ::synaptic_core::SynapticError> {
                #(#field_clones)*
                #impl_fn_name(#(#fidents,)* messages).await
            }
        }

        #factory
    })
}

// ---------------------------------------------------------------------------
// #[wrap_model_call]
// ---------------------------------------------------------------------------

/// Expand `#[wrap_model_call]` on an async function.
///
/// Supports `#[field]` parameters for stateful middleware.
pub fn expand_wrap_model_call(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[wrap_model_call] does not accept arguments",
        ));
    }

    let func: ItemFn = parse2(item)?;
    let (fields, clean_func) = extract_field_params(&func)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let struct_name = format_ident!("{}Middleware", to_pascal_case(&fn_name.to_string()));
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    let mut impl_func = clean_func;
    impl_func.sig.ident = impl_fn_name.clone();
    impl_func
        .attrs
        .retain(|a| !a.path().is_ident("wrap_model_call"));

    let struct_def = gen_middleware_struct(vis, &struct_name, &fields);
    let factory = gen_middleware_factory(vis, fn_name, &struct_name, &fields);
    let field_clones = gen_field_clones(&fields);
    let fidents = field_idents(&fields);

    Ok(quote! {
        #impl_func

        #struct_def

        #[::async_trait::async_trait]
        impl ::synaptic_middleware::AgentMiddleware for #struct_name {
            async fn wrap_model_call(
                &self,
                request: ::synaptic_middleware::ModelRequest,
                next: &dyn ::synaptic_middleware::ModelCaller,
            ) -> Result<::synaptic_middleware::ModelResponse, ::synaptic_core::SynapticError> {
                #(#field_clones)*
                #impl_fn_name(#(#fidents,)* request, next).await
            }
        }

        #factory
    })
}

// ---------------------------------------------------------------------------
// #[wrap_tool_call]
// ---------------------------------------------------------------------------

/// Expand `#[wrap_tool_call]` on an async function.
///
/// Supports `#[field]` parameters for stateful middleware.
pub fn expand_wrap_tool_call(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[wrap_tool_call] does not accept arguments",
        ));
    }

    let func: ItemFn = parse2(item)?;
    let (fields, clean_func) = extract_field_params(&func)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let struct_name = format_ident!("{}Middleware", to_pascal_case(&fn_name.to_string()));
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    let mut impl_func = clean_func;
    impl_func.sig.ident = impl_fn_name.clone();
    impl_func
        .attrs
        .retain(|a| !a.path().is_ident("wrap_tool_call"));

    let struct_def = gen_middleware_struct(vis, &struct_name, &fields);
    let factory = gen_middleware_factory(vis, fn_name, &struct_name, &fields);
    let field_clones = gen_field_clones(&fields);
    let fidents = field_idents(&fields);

    Ok(quote! {
        #impl_func

        #struct_def

        #[::async_trait::async_trait]
        impl ::synaptic_middleware::AgentMiddleware for #struct_name {
            async fn wrap_tool_call(
                &self,
                request: ::synaptic_middleware::ToolCallRequest,
                next: &dyn ::synaptic_middleware::ToolCaller,
            ) -> Result<::serde_json::Value, ::synaptic_core::SynapticError> {
                #(#field_clones)*
                #impl_fn_name(#(#fidents,)* request, next).await
            }
        }

        #factory
    })
}

// ---------------------------------------------------------------------------
// #[dynamic_prompt]
// ---------------------------------------------------------------------------

/// Expand `#[dynamic_prompt]` on a (non-async) function.
///
/// Produces a middleware whose `before_model` sets `request.system_prompt`.
///
/// Supports `#[field]` parameters for stateful middleware.
pub fn expand_dynamic_prompt(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[dynamic_prompt] does not accept arguments",
        ));
    }

    let func: ItemFn = parse2(item)?;
    let (fields, clean_func) = extract_field_params(&func)?;

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let struct_name = format_ident!("{}Middleware", to_pascal_case(&fn_name.to_string()));
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    let mut impl_func = clean_func;
    impl_func.sig.ident = impl_fn_name.clone();
    impl_func
        .attrs
        .retain(|a| !a.path().is_ident("dynamic_prompt"));

    let struct_def = gen_middleware_struct(vis, &struct_name, &fields);
    let factory = gen_middleware_factory(vis, fn_name, &struct_name, &fields);
    let field_clones = gen_field_clones(&fields);
    let fidents = field_idents(&fields);

    // dynamic_prompt is special: the user function takes &[Message]
    // but the trait method receives &mut ModelRequest. We bind
    // `messages` from `request.messages` so the impl_fn gets &[Message].
    Ok(quote! {
        #impl_func

        #struct_def

        #[::async_trait::async_trait]
        impl ::synaptic_middleware::AgentMiddleware for #struct_name {
            async fn before_model(
                &self,
                request: &mut ::synaptic_middleware::ModelRequest,
            ) -> Result<(), ::synaptic_core::SynapticError> {
                #(#field_clones)*
                let prompt = #impl_fn_name(#(#fidents,)* &request.messages);
                request.system_prompt = Some(prompt);
                Ok(())
            }
        }

        #factory
    })
}
