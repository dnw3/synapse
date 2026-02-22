use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::Parser, parse2, punctuated::Punctuated, Attribute, Expr, FnArg, ItemFn, Lit, Meta, Pat,
    PatType, ReturnType, Token, Type,
};

use crate::paths;

// ---------------------------------------------------------------------------
// Attribute-level config: #[tool(name = "...", description = "...")]
// ---------------------------------------------------------------------------

struct ToolAttr {
    name: Option<String>,
    description: Option<String>,
}

fn parse_tool_attr(attr: TokenStream) -> syn::Result<ToolAttr> {
    let mut name = None;
    let mut description = None;

    if attr.is_empty() {
        return Ok(ToolAttr { name, description });
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
                        "description" => description = Some(lit_str.value()),
                        _ => {
                            return Err(syn::Error::new_spanned(
                                &nv.path,
                                format!("unknown tool attribute: `{}`", key),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(ToolAttr { name, description })
}

// ---------------------------------------------------------------------------
// Per-parameter metadata
// ---------------------------------------------------------------------------

enum InjectKind {
    State,
    Store,
    ToolCallId,
}

struct ParamInfo {
    name: String,
    ty: Type,
    is_option: bool,
    default_value: Option<Expr>,
    inject: Option<InjectKind>,
    is_field: bool,
    is_args: bool,
    doc: Option<String>,
}

fn extract_doc_comment(attrs: &[Attribute]) -> Option<String> {
    let mut lines = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(s) = &expr_lit.lit {
                        lines.push(s.value().trim().to_string());
                    }
                }
            }
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" "))
    }
}

fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident == "Option";
        }
    }
    false
}

fn inner_option_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

fn parse_param_info(pat_type: &PatType) -> syn::Result<ParamInfo> {
    let name = if let Pat::Ident(pi) = &*pat_type.pat {
        pi.ident.to_string()
    } else {
        return Err(syn::Error::new_spanned(
            &pat_type.pat,
            "expected a simple identifier",
        ));
    };

    let ty = (*pat_type.ty).clone();
    let is_option = is_option_type(&ty);
    let mut default_value = None;
    let mut inject = None;
    let mut is_field = false;
    let mut is_args = false;
    let doc = extract_doc_comment(&pat_type.attrs);

    for attr in &pat_type.attrs {
        // #[default = expr]
        if attr.path().is_ident("default") {
            if let Meta::NameValue(nv) = &attr.meta {
                default_value = Some(nv.value.clone());
            }
        }
        // #[field]
        if attr.path().is_ident("field") {
            is_field = true;
        }
        // #[args]
        if attr.path().is_ident("args") {
            is_args = true;
        }
        // #[inject(state)] / #[inject(store)] / #[inject(tool_call_id)]
        if attr.path().is_ident("inject") {
            let tokens: TokenStream = attr.parse_args()?;
            let kind_str = tokens.to_string();
            inject = Some(match kind_str.as_str() {
                "state" => InjectKind::State,
                "store" => InjectKind::Store,
                "tool_call_id" => InjectKind::ToolCallId,
                _ => {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "expected inject(state), inject(store), or inject(tool_call_id)",
                    ))
                }
            });
        }
    }

    if is_field && inject.is_some() {
        return Err(syn::Error::new_spanned(
            &pat_type.pat,
            "#[field] and #[inject] cannot be used on the same parameter",
        ));
    }

    if is_args && inject.is_some() {
        return Err(syn::Error::new_spanned(
            &pat_type.pat,
            "#[args] and #[inject] cannot be used on the same parameter",
        ));
    }

    if is_args && is_field {
        return Err(syn::Error::new_spanned(
            &pat_type.pat,
            "#[args] and #[field] cannot be used on the same parameter",
        ));
    }

    Ok(ParamInfo {
        name,
        ty,
        is_option,
        default_value,
        inject,
        is_field,
        is_args,
        doc,
    })
}

// ---------------------------------------------------------------------------
// Type → JSON schema string (compile-time)
// ---------------------------------------------------------------------------

fn type_to_json_schema(ty: &Type) -> TokenStream {
    let inner = if is_option_type(ty) {
        inner_option_type(ty).unwrap_or(ty)
    } else {
        ty
    };

    if let Type::Path(tp) = inner {
        if let Some(seg) = tp.path.segments.last() {
            let ident = seg.ident.to_string();
            match ident.as_str() {
                "String" | "str" => {
                    return quote! { ::serde_json::json!({"type": "string"}) };
                }
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
                | "u128" | "usize" => {
                    return quote! { ::serde_json::json!({"type": "integer"}) };
                }
                "f32" | "f64" => {
                    return quote! { ::serde_json::json!({"type": "number"}) };
                }
                "bool" => {
                    return quote! { ::serde_json::json!({"type": "boolean"}) };
                }
                "Vec" => {
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        if let Some(syn::GenericArgument::Type(elem_ty)) = args.args.first() {
                            let items = type_to_json_schema(elem_ty);
                            return quote! {
                                ::serde_json::json!({"type": "array", "items": #items})
                            };
                        }
                    }
                    return quote! { ::serde_json::json!({"type": "array"}) };
                }
                "Value" => {
                    return quote! { ::serde_json::json!({"type": "object"}) };
                }
                _ => {}
            }
        }
    }
    // Fallback: unknown type
    unknown_type_schema(inner)
}

#[cfg(feature = "schemars")]
fn unknown_type_schema(inner: &Type) -> TokenStream {
    let core_crate = paths::core_path();
    quote! {
        {
            let __schema = #core_crate::schemars::generate::SchemaGenerator::default()
                .into_root_schema_for::<#inner>();
            let mut __val = ::serde_json::to_value(&__schema)
                .unwrap_or(::serde_json::json!({"type": "object"}));
            if let Some(__obj) = __val.as_object_mut() {
                __obj.remove("$schema");
            }
            __val
        }
    }
}

#[cfg(not(feature = "schemars"))]
fn unknown_type_schema(_inner: &Type) -> TokenStream {
    quote! { ::serde_json::json!({"type": "object"}) }
}

// ---------------------------------------------------------------------------
// Code generation for parameter deserialization
// ---------------------------------------------------------------------------

fn gen_param_deser(param: &ParamInfo) -> TokenStream {
    let name_str = &param.name;
    let ident = format_ident!("{}", &param.name);
    let ty = &param.ty;
    let core_crate = paths::core_path();

    if param.is_option {
        let inner_ty = inner_option_type(ty).unwrap();
        quote! {
            let #ident: #ty = match __args.get(#name_str) {
                Some(::serde_json::Value::Null) | None => None,
                Some(__v) => Some(
                    ::serde_json::from_value::<#inner_ty>(__v.clone())
                        .map_err(|__e| #core_crate::SynapticError::Tool(
                            format!("invalid parameter '{}': {}", #name_str, __e)
                        ))?
                ),
            };
        }
    } else if let Some(ref default_expr) = param.default_value {
        quote! {
            let #ident: #ty = match __args.get(#name_str) {
                Some(::serde_json::Value::Null) | None => #default_expr,
                Some(__v) => ::serde_json::from_value(__v.clone())
                    .map_err(|__e| #core_crate::SynapticError::Tool(
                        format!("invalid parameter '{}': {}", #name_str, __e)
                    ))?,
            };
        }
    } else {
        quote! {
            let #ident: #ty = ::serde_json::from_value(
                __args.get(#name_str)
                    .cloned()
                    .ok_or_else(|| #core_crate::SynapticError::Tool(
                        format!("missing required parameter: {}", #name_str)
                    ))?
            ).map_err(|__e| #core_crate::SynapticError::Tool(
                format!("invalid parameter '{}': {}", #name_str, __e)
            ))?;
        }
    }
}

fn gen_inject_deser(param: &ParamInfo) -> TokenStream {
    let ident = format_ident!("{}", &param.name);
    let ty = &param.ty;
    let core_crate = paths::core_path();

    match param.inject.as_ref().unwrap() {
        InjectKind::State => {
            quote! {
                let #ident: #ty = ::serde_json::from_value(
                    __runtime.state.clone().unwrap_or(::serde_json::Value::Null)
                ).map_err(|__e| #core_crate::SynapticError::Tool(
                    format!("failed to inject state: {}", __e)
                ))?;
            }
        }
        InjectKind::Store => {
            quote! {
                let #ident: #ty = __runtime.store.clone()
                    .ok_or_else(|| #core_crate::SynapticError::Tool(
                        "inject(store): no store in runtime".into()
                    ))?;
            }
        }
        InjectKind::ToolCallId => {
            quote! {
                let #ident: #ty = __runtime.tool_call_id.clone();
            }
        }
    }
}

fn gen_field_deser(param: &ParamInfo) -> TokenStream {
    let ident = format_ident!("{}", &param.name);
    quote! {
        let #ident = self.#ident.clone();
    }
}

fn gen_args_deser(param: &ParamInfo) -> TokenStream {
    let ident = format_ident!("{}", &param.name);
    let ty = &param.ty;
    quote! {
        let #ident: #ty = __args;
    }
}

// ---------------------------------------------------------------------------
// Main expansion
// ---------------------------------------------------------------------------

pub fn expand_tool(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let tool_attr = parse_tool_attr(attr)?;
    let func: ItemFn = parse2(item)?;

    // Extract function name & visibility
    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let struct_name = format_ident!("{}Tool", to_pascal_case(&fn_name.to_string()));
    let impl_fn_name = format_ident!("{}_impl", fn_name);

    // Extract doc comment from the function for description
    let fn_doc = extract_doc_comment(&func.attrs);
    let tool_name_str = tool_attr.name.unwrap_or_else(|| fn_name.to_string());
    let tool_desc_str = tool_attr.description.or(fn_doc).unwrap_or_default();

    // Parse parameters
    let mut params: Vec<ParamInfo> = Vec::new();
    for arg in &func.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            params.push(parse_param_info(pat_type)?);
        }
    }

    let has_inject = params.iter().any(|p| p.inject.is_some());
    let has_field = params.iter().any(|p| p.is_field);
    let args_count = params.iter().filter(|p| p.is_args).count();

    if args_count > 1 {
        return Err(syn::Error::new_spanned(
            &func.sig,
            "at most one parameter can be marked with #[args]",
        ));
    }

    // Build JSON schema properties & required list
    let schema_params: Vec<&ParamInfo> = params
        .iter()
        .filter(|p| p.inject.is_none() && !p.is_field && !p.is_args)
        .collect();

    let mut prop_entries = Vec::new();
    let mut required_entries = Vec::new();

    for p in &schema_params {
        let name_str = &p.name;
        let schema = type_to_json_schema(&p.ty);

        let with_desc = if let Some(ref doc) = p.doc {
            quote! {
                {
                    let mut __s = #schema;
                    if let Some(__obj) = __s.as_object_mut() {
                        __obj.insert("description".into(), ::serde_json::Value::String(#doc.into()));
                    }
                    __s
                }
            }
        } else {
            schema
        };

        let with_default = if let Some(ref def) = p.default_value {
            let def_str = quote!(#def).to_string();
            quote! {
                {
                    let mut __s = #with_desc;
                    if let Some(__obj) = __s.as_object_mut() {
                        // Try to parse as JSON, fallback to string
                        let __default_val: ::serde_json::Value =
                            ::serde_json::from_str(#def_str).unwrap_or(
                                ::serde_json::Value::String(#def_str.into())
                            );
                        __obj.insert("default".into(), __default_val);
                    }
                    __s
                }
            }
        } else {
            with_desc
        };

        prop_entries.push(quote! {
            __props.insert(#name_str.to_string(), #with_default);
        });

        // Required: not Option, not has default
        if !p.is_option && p.default_value.is_none() {
            required_entries.push(quote! { #name_str.to_string() });
        }
    }

    // Build deserialization code
    let deser_stmts: Vec<TokenStream> = params
        .iter()
        .map(|p| {
            if p.is_field {
                gen_field_deser(p)
            } else if p.is_args {
                gen_args_deser(p)
            } else if p.inject.is_some() {
                gen_inject_deser(p)
            } else {
                gen_param_deser(p)
            }
        })
        .collect();

    let param_idents: Vec<_> = params
        .iter()
        .map(|p| format_ident!("{}", &p.name))
        .collect();

    // Extract function body and return type
    let fn_body = &func.block;
    let fn_ret = &func.sig.output;
    let asyncness = &func.sig.asyncness;

    // Strip attributes from parameters for the impl function
    let clean_params: Vec<TokenStream> = params
        .iter()
        .map(|p| {
            let ident = format_ident!("{}", &p.name);
            let ty = &p.ty;
            quote! { #ident: #ty }
        })
        .collect();

    // Determine return type (extract T from Result<T, SynapticError>)
    let _ret_type = match fn_ret {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => quote! { #ty },
    };

    // Collect field params for struct generation
    let field_params: Vec<&ParamInfo> = params.iter().filter(|p| p.is_field).collect();

    // Struct definition: empty or with fields
    let struct_def = if has_field {
        let field_defs: Vec<TokenStream> = field_params
            .iter()
            .map(|p| {
                let ident = format_ident!("{}", &p.name);
                let ty = &p.ty;
                quote! { #ident: #ty }
            })
            .collect();
        quote! { #vis struct #struct_name { #(#field_defs),* } }
    } else {
        quote! { #vis struct #struct_name; }
    };

    // Factory function params and construction
    let factory_params: Vec<TokenStream> = field_params
        .iter()
        .map(|p| {
            let ident = format_ident!("{}", &p.name);
            let ty = &p.ty;
            quote! { #ident: #ty }
        })
        .collect();

    let factory_construction = if has_field {
        let field_inits: Vec<TokenStream> = field_params
            .iter()
            .map(|p| {
                let ident = format_ident!("{}", &p.name);
                quote! { #ident }
            })
            .collect();
        quote! { #struct_name { #(#field_inits),* } }
    } else {
        quote! { #struct_name }
    };

    // Generate parameters() body: None if no schema params, Some(...) otherwise
    let parameters_body = if schema_params.is_empty() {
        quote! { None }
    } else {
        quote! {
            let mut __props = ::serde_json::Map::new();
            #(#prop_entries)*
            let __required: Vec<String> = vec![#(#required_entries),*];
            Some(::serde_json::json!({
                "type": "object",
                "properties": ::serde_json::Value::Object(__props),
                "required": __required,
            }))
        }
    };

    let core_crate = paths::core_path();

    if has_inject {
        // Generate RuntimeAwareTool impl
        Ok(quote! {
            #asyncness fn #impl_fn_name(#(#clean_params),*) #fn_ret
                #fn_body

            #struct_def

            #[::async_trait::async_trait]
            impl #core_crate::RuntimeAwareTool for #struct_name {
                fn name(&self) -> &'static str {
                    #tool_name_str
                }

                fn description(&self) -> &'static str {
                    #tool_desc_str
                }

                fn parameters(&self) -> Option<::serde_json::Value> {
                    #parameters_body
                }

                async fn call_with_runtime(
                    &self,
                    __args: ::serde_json::Value,
                    __runtime: #core_crate::ToolRuntime,
                ) -> Result<::serde_json::Value, #core_crate::SynapticError> {
                    #(#deser_stmts)*
                    let __result = #impl_fn_name(#(#param_idents),*).await?;
                    ::serde_json::to_value(__result)
                        .map_err(|__e| #core_crate::SynapticError::Tool(
                            format!("failed to serialize result: {}", __e)
                        ))
                }
            }

            #vis fn #fn_name(#(#factory_params),*) -> ::std::sync::Arc<dyn #core_crate::RuntimeAwareTool> {
                ::std::sync::Arc::new(#factory_construction)
            }
        })
    } else {
        // Generate plain Tool impl
        Ok(quote! {
            #asyncness fn #impl_fn_name(#(#clean_params),*) #fn_ret
                #fn_body

            #struct_def

            #[::async_trait::async_trait]
            impl #core_crate::Tool for #struct_name {
                fn name(&self) -> &'static str {
                    #tool_name_str
                }

                fn description(&self) -> &'static str {
                    #tool_desc_str
                }

                fn parameters(&self) -> Option<::serde_json::Value> {
                    #parameters_body
                }

                async fn call(&self, __args: ::serde_json::Value) -> Result<::serde_json::Value, #core_crate::SynapticError> {
                    #(#deser_stmts)*
                    let __result = #impl_fn_name(#(#param_idents),*).await?;
                    ::serde_json::to_value(__result)
                        .map_err(|__e| #core_crate::SynapticError::Tool(
                            format!("failed to serialize result: {}", __e)
                        ))
                }
            }

            #vis fn #fn_name(#(#factory_params),*) -> ::std::sync::Arc<dyn #core_crate::Tool> {
                ::std::sync::Arc::new(#factory_construction)
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
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
