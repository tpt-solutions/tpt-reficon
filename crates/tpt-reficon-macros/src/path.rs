//! Resolves the runtime crate's path in the *user's* crate so generated code
//! can reference `tpt-reficon-core` under whatever name Cargo renamed it to.

use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::Ident;

/// Path prefix for the runtime crate, e.g. `::tpt_reficon_core`.
///
/// Resolution order: a `tpt-reficon-core` dependency, else a `tpt-reficon`
/// dependency (whose `core` submodule re-exports it), else the conventional
/// `::tpt_reficon_core` path.
pub fn core_crate() -> TokenStream {
    match crate_name("tpt-reficon-core") {
        Ok(FoundCrate::Name(name)) => ident_path(&name),
        // FoundCrate::Itself cannot occur: this is a proc-macro crate.
        _ => match crate_name("tpt-reficon") {
            Ok(FoundCrate::Name(name)) => {
                let crate_ident = ident_path(&name);
                quote!(#crate_ident::core)
            }
            _ => quote!(::tpt_reficon_core),
        },
    }
}

fn ident_path(name: &str) -> TokenStream {
    let ident = Ident::new(name, Span::call_site());
    quote!(::#ident)
}
