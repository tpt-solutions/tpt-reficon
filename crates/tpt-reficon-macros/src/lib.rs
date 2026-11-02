//! `tpt-reficon-macros` — the proc-macro frontend.
//!
//! Translates user predicates straight into the [`tpt-reficon-verifier`]
//! constraint IR (not SMT-LIB2 — see the deviation note in the workspace
//! `CHANGELOG.md`) and discharges them at macro-expansion time.
//!
//! Supported today:
//!
//! - `#[derive(Refine)]` on single-field (newtype) structs, with
//!   `#[requires(...)]` / `#[ensures(...)]` predicates over the inner value.
//!   Contradictory refinements are compile errors; refinements the type's
//!   domain already implies are compiled out entirely (zero-cost elision);
//!   everything else gets an exact runtime check in `new` / `try_new`.
//! - Predicates may combine linear comparisons with `&&` and `||` (lowered to
//!   disjunctive normal form: a disjunction is unsatisfiable only when every
//!   alternative is, and the domain elides the check when it proves an
//!   alternative).
//! - Integer-domain completeness: for primitive integer inner types, an exact
//!   single-variable integer-feasibility analysis runs per alternative against
//!   the type's domain, so rational-only refinements (`value > 5 && value < 6`,
//!   or `value > 255` on `u8`) become compile errors rather than
//!   always-panicking constructors.

#![warn(missing_docs)]

extern crate proc_macro;

mod attrs;
mod domain;
mod expand;
mod ffi;
mod integer;
mod path;
mod predicate;

use syn::parse_macro_input;

/// Derives a refinement type.
///
/// # Supported attributes
///
/// - `#[requires(<expr>)]` — refine with a predicate the inner value must
///   satisfy. Repeatable; multiple predicates are conjoined. Predicates are
///   boolean combinations (`&&`, `||`) of linear comparisons over the refined
///   variable, e.g. `value > 0 && value <= 100` or `value == 0 || value == 1`.
/// - `#[ensures(<expr>)]` — accepted as an additional conjoined predicate.
/// - `#[refine(variable = "name")]` — names the predicate variable that stands
///   for the inner value. Defaults to the single field's name, or `value` for
///   tuple structs.
#[proc_macro_derive(Refine, attributes(requires, ensures, refine))]
pub fn derive_refine(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match expand::expand_refine(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Proves that a `#[repr(C)]` struct mirrors a C struct declared in a header.
///
/// # Arguments
///
/// - `bridge = "c"` or `bridge = "go"` — the foreign side. Both read a C
///   header; a Go struct must be declared as a C struct in the cgo preamble.
/// - `header = "..."` — path to the header, relative to the crate's
///   `CARGO_MANIFEST_DIR`.
/// - `c_name = "..."` — optional; the C struct name when it differs from the
///   Rust struct's name.
///
/// Requires the `ffi-bindgen` feature (`tpt-reficon/ffi` from the facade) and
/// libclang at build time. See `docs/design/go-header-ingestion.md` for the Go
/// bridge and the crate docs for the supported field types.
#[proc_macro_attribute]
pub fn ffi_contract(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match ffi::expand(attr.into(), item.into()) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
