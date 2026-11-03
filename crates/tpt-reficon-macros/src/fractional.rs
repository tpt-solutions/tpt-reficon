//! `#[fractional_borrow]`: typed, lock-free shared state with fractional
//! permissions (prototype; compiled only with the `unstable` feature).
//!
//! ```ignore
//! #[fractional_borrow]
//! #[derive(Debug, Clone, Copy)]
//! pub struct Counters {
//!     hits: u64,
//!     ok: bool,
//! }
//! ```
//!
//! The struct is emitted unchanged, as a plain snapshot. The macro adds a
//! `CountersShared` type with one [`Shared`](tpt_reficon_core::fractional::Shared)
//! cell per field, plus per-field accessors:
//!
//! - `CountersShared::new(counters)` moves a snapshot into shared cells;
//! - `try_read_hits()` takes one unit of read permission on `hits`;
//! - `try_write_hits()` takes the whole permission on `hits`.
//!
//! Permission is per field. A writer on `hits` does not block readers of `ok`,
//! and the non-interference condition for each field is the one checked by
//! `tpt_reficon_verifier::permission::check_configuration`. Design notes are in
//! `docs/design/fractional-borrows.md`.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Fields, Ident, ItemStruct, Type};

use crate::path;

/// Field types the shared cell can hold. Matches `tpt_reficon_core::fractional::Bits`.
const SUPPORTED: &[&str] = &["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "bool"];

/// Expands `#[fractional_borrow]` on a named-field struct.
pub fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "`#[fractional_borrow]` takes no arguments",
        ));
    }
    let item: ItemStruct = syn::parse2(item)?;
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(
            item.generics.span(),
            "`#[fractional_borrow]` does not support generic structs",
        ));
    }
    let Fields::Named(named) = &item.fields else {
        return Err(syn::Error::new(
            item.span(),
            "`#[fractional_borrow]` needs a struct with named fields",
        ));
    };
    if named.named.is_empty() {
        return Err(syn::Error::new(
            item.span(),
            "`#[fractional_borrow]` needs at least one field",
        ));
    }

    let mut names: Vec<Ident> = Vec::new();
    let mut types: Vec<Type> = Vec::new();
    for field in &named.named {
        let ident = field.ident.clone().ok_or_else(|| {
            syn::Error::new(field.span(), "`#[fractional_borrow]` needs named fields")
        })?;
        if !is_supported(&field.ty) {
            return Err(syn::Error::new(
                field.ty.span(),
                "`#[fractional_borrow]` fields must be `bool` or a primitive integer of at most 64 bits",
            ));
        }
        names.push(ident);
        types.push(field.ty.clone());
    }

    let core = path::core_crate();
    let vis = &item.vis;
    let name = &item.ident;
    let shared = format_ident!("{}Shared", name);
    let try_read: Vec<Ident> = names
        .iter()
        .map(|n| format_ident!("try_read_{}", n))
        .collect();
    let try_write: Vec<Ident> = names
        .iter()
        .map(|n| format_ident!("try_write_{}", n))
        .collect();

    Ok(quote! {
        #item

        /// Shared, lock-free state for the fields of the snapshot struct, with
        /// one fractional permission per field.
        #vis struct #shared {
            #( #names: #core::fractional::Shared<#types>, )*
        }

        impl #shared {
            /// Moves a snapshot into shared cells.
            pub fn new(initial: #name) -> Self {
                Self {
                    #( #names: #core::fractional::Shared::new(initial.#names), )*
                }
            }

            #(
                /// Takes one unit of read permission on this field.
                pub fn #try_read(&self) -> ::core::option::Option<#core::fractional::Read<'_, #types>> {
                    self.#names.try_read()
                }

                /// Takes the whole permission on this field.
                pub fn #try_write(&self) -> ::core::option::Option<#core::fractional::Write<'_, #types>> {
                    self.#names.try_write()
                }
            )*
        }
    })
}

fn is_supported(ty: &Type) -> bool {
    match ty {
        Type::Path(p) if p.qself.is_none() && p.path.segments.len() == 1 => {
            let ident = p.path.segments[0].ident.to_string();
            SUPPORTED.contains(&ident.as_str())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(src: &str) -> String {
        let item: TokenStream = src.parse().unwrap();
        expand(TokenStream::new(), item).unwrap_err().to_string()
    }

    #[test]
    fn expands_one_cell_and_accessor_pair_per_field() {
        let item: TokenStream = "pub struct C { a: u8, b: bool }".parse().unwrap();
        let out = expand(TokenStream::new(), item).unwrap().to_string();
        assert!(out.contains("CShared"), "{out}");
        assert!(out.contains("try_read_a"), "{out}");
        assert!(out.contains("try_write_b"), "{out}");
    }

    #[test]
    fn non_primitive_field_is_rejected() {
        let msg = err("struct C { name: String }");
        assert!(
            msg.contains("must be `bool` or a primitive integer"),
            "{msg}"
        );
    }

    #[test]
    fn pointer_sized_and_128_bit_fields_are_rejected() {
        // The atomic cell holds at most 64 bits. `usize`/`isize` have no fixed
        // width across targets, and `u128` does not fit an `AtomicU64`.
        for ty in ["usize", "isize", "u128", "i128"] {
            let msg = err(&format!("struct C {{ a: {ty} }}"));
            assert!(
                msg.contains("must be `bool` or a primitive integer"),
                "{ty}: {msg}"
            );
        }
    }

    #[test]
    fn wide_float_field_is_rejected() {
        let msg = err("struct C { x: f64 }");
        assert!(
            msg.contains("must be `bool` or a primitive integer"),
            "{msg}"
        );
    }

    #[test]
    fn generic_struct_is_rejected() {
        let msg = err("struct C<T> { a: u8, t: T }");
        assert!(msg.contains("does not support generic"), "{msg}");
    }

    #[test]
    fn tuple_struct_is_rejected() {
        let msg = err("struct C(u8);");
        assert!(msg.contains("named fields"), "{msg}");
    }
}
