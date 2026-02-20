//! Path resolution helpers for proc-macro generated code.
//!
//! Uses `proc-macro-crate` to detect whether the downstream crate depends on the
//! `synaptic` facade or on the individual sub-crates (`synaptic-core`, etc.) directly.
//!
//! - If the downstream crate has `synaptic` in its Cargo.toml, emitted code
//!   uses `::synaptic::synaptic_core`, `::synaptic::synaptic_runnables`, etc.
//! - Otherwise, emitted code uses `::synaptic_core`, `::synaptic_runnables`, etc.

use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;

/// Returns the token path for `synaptic_core`.
///
/// Checks if the user has the `synaptic` facade crate; if so, routes through it.
pub fn core_path() -> TokenStream {
    match crate_name("synaptic") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let ident = proc_macro2::Ident::new(&name, proc_macro2::Span::call_site());
            quote! { ::#ident::synaptic_core }
        }
        Err(_) => quote! { ::synaptic_core },
    }
}

/// Returns the token path for `synaptic_runnables`.
pub fn runnables_path() -> TokenStream {
    match crate_name("synaptic") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let ident = proc_macro2::Ident::new(&name, proc_macro2::Span::call_site());
            quote! { ::#ident::synaptic_runnables }
        }
        Err(_) => quote! { ::synaptic_runnables },
    }
}

/// Returns the token path for `synaptic_middleware`.
pub fn middleware_path() -> TokenStream {
    match crate_name("synaptic") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let ident = proc_macro2::Ident::new(&name, proc_macro2::Span::call_site());
            quote! { ::#ident::synaptic_middleware }
        }
        Err(_) => quote! { ::synaptic_middleware },
    }
}
