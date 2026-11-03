//! `ffi_wrappers!`: safe Rust wrappers around C functions declared in a header.
//!
//! ```ignore
//! ffi_wrappers! {
//!     header = "payment.h";
//!     bridge = "c";
//!     #[requires(amount > 0)]
//!     pub fn process_payment(amount: i32) -> i32;
//! }
//! ```
//!
//! For each declaration the macro does three things:
//!
//! 1. **Checks the signature against the header.** The function is read from
//!    bindgen's output, and the parameter count, every parameter type, and the
//!    return type must match the Rust declaration exactly, after mapping the
//!    `c_*` aliases to Rust primitives. Pointer mutability is part of the
//!    comparison.
//! 2. **Declares the raw symbol** in an `extern "C"` block under a private
//!    name, linked to the C name with `#[link_name]`.
//! 3. **Generates a safe wrapper** with the user's name. Its `#[requires]` and
//!    `#[ensures]` attributes go through `#[contract]`, so the predicates are
//!    checked before the unsafe call, and only then does the call run.
//!
//! The raw call is the one `unsafe` block, and it is confined to the wrapper
//! with `#[allow(unsafe_code)]`. A crate that forbids `unsafe_code` cannot use
//! this macro. See `docs/design/ffi-wrappers.md`.

use proc_macro2::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::{ForeignItemFn, Ident, LitStr, Token};

#[cfg(feature = "ffi-bindgen")]
use quote::{format_ident, quote};
#[cfg(feature = "ffi-bindgen")]
use syn::spanned::Spanned;
#[cfg(feature = "ffi-bindgen")]
use syn::{Attribute, FnArg, ForeignItem, Item, ReturnType, Type};

/// Parsed `ffi_wrappers!` input: the header, the bridge, and the declarations.
#[cfg_attr(not(feature = "ffi-bindgen"), allow(dead_code))]
struct WrapperInput {
    header: LitStr,
    fns: Vec<ForeignItemFn>,
}

impl Parse for WrapperInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut header = None;
        let mut bridge = None;
        while input.peek(Ident) && input.peek2(Token![=]) {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;
            input.parse::<Token![;]>()?;
            let slot = match key.to_string().as_str() {
                "header" => &mut header,
                "bridge" => &mut bridge,
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown `ffi_wrappers` key `{other}`; expected `header` or `bridge`"
                        ),
                    ))
                }
            };
            if slot.replace(value).is_some() {
                return Err(syn::Error::new(key.span(), "duplicate `ffi_wrappers` key"));
            }
        }
        let header = header.ok_or_else(|| {
            syn::Error::new(input.span(), "`ffi_wrappers!` requires `header = \"...\";`")
        })?;
        if let Some(bridge) = bridge {
            match bridge.value().as_str() {
                "c" | "go" => {}
                _ => {
                    return Err(syn::Error::new(
                        bridge.span(),
                        "`ffi_wrappers` bridge must be \"c\" or \"go\"",
                    ))
                }
            }
        }
        let mut fns = Vec::new();
        while !input.is_empty() {
            fns.push(input.parse::<ForeignItemFn>()?);
        }
        Ok(WrapperInput { header, fns })
    }
}

/// Expands `ffi_wrappers!`.
#[cfg(feature = "ffi-bindgen")]
pub fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let parsed: WrapperInput = syn::parse2(input)?;
    let ast = super::ffi::bindgen_ast(&parsed.header, false)?;

    let mut out = TokenStream::new();
    for decl in &parsed.fns {
        out.extend(expand_one(decl, &ast)?);
    }
    Ok(out)
}

#[cfg(not(feature = "ffi-bindgen"))]
pub fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    // Parse anyway, so that syntax errors are reported the same way in both
    // configurations.
    let _parsed: WrapperInput = syn::parse2(input)?;
    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "`ffi_wrappers!` needs the `ffi-bindgen` feature of `tpt-reficon-macros` (or `tpt-reficon/ffi`)",
    ))
}

#[cfg(feature = "ffi-bindgen")]
fn expand_one(decl: &ForeignItemFn, ast: &syn::File) -> syn::Result<TokenStream> {
    let sig = &decl.sig;
    if sig.variadic.is_some() || sig.abi.is_some() || !sig.generics.params.is_empty() {
        return Err(syn::Error::new(
            sig.span(),
            "`ffi_wrappers!` declarations must be plain, non-generic, non-variadic functions",
        ));
    }

    let c_sig = find_c_function(ast, &sig.ident.to_string()).ok_or_else(|| {
        syn::Error::new(
            sig.ident.span(),
            format!("function `{}` is not declared in the header", sig.ident),
        )
    })?;
    check_signature(sig, c_sig)?;

    let name = &sig.ident;
    let raw = format_ident!("__tpt_raw_{}", name);
    let c_name = name.to_string();

    // Raw parameters take their types from the declaration; the wrapper keeps
    // the user's names so that predicates can refer to them.
    let mut raw_params = Vec::new();
    let mut call_args = Vec::new();
    let mut wrapper_args = Vec::new();
    for arg in &sig.inputs {
        let FnArg::Typed(pt) = arg else {
            return Err(syn::Error::new(
                arg.span(),
                "`ffi_wrappers!` does not support `self`",
            ));
        };
        let ty = &pt.ty;
        let pat = &pt.pat;
        let raw_name = format_ident!("__arg{}", raw_params.len());
        raw_params.push(quote!(#raw_name: #ty));
        call_args.push(quote!(#pat));
        wrapper_args.push(quote!(#pat: #ty));
    }
    let ret = &sig.output;
    let vis = &decl.vis;
    let user_attrs: Vec<&Attribute> = decl.attrs.iter().collect();
    let is_contract = |a: &Attribute| a.path().is_ident("requires") || a.path().is_ident("ensures");
    let contract_attrs: Vec<&Attribute> = user_attrs
        .iter()
        .copied()
        .filter(|a| is_contract(a))
        .collect();
    let doc_attrs: Vec<&Attribute> = user_attrs
        .iter()
        .copied()
        .filter(|a| !is_contract(a))
        .collect();
    let has_contract = !contract_attrs.is_empty();
    // A raw pointer argument can dangle or be null, so calling the function
    // with one is the caller's obligation: the public wrapper is `unsafe`.
    let has_pointer = sig
        .inputs
        .iter()
        .any(|arg| matches!(arg, FnArg::Typed(pt) if matches!(*pt.ty, Type::Ptr(_))));

    let wrapper = if !has_contract && !has_pointer {
        quote! {
            #(#doc_attrs)*
            #[allow(unsafe_code)]
            #vis fn #name(#(#wrapper_args),*) #ret {
                // SAFETY: the signature was checked against the header above.
                unsafe { #raw(#(#call_args),*) }
            }
        }
    } else {
        // The checked body is a private, safe function. `#[contract]` rejects
        // `unsafe fn`, so the contract cannot sit on the public wrapper when
        // that wrapper is `unsafe`. The public wrapper only forwards.
        let checked = format_ident!("__tpt_checked_{}", name);
        let inner = quote! {
            #(#contract_attrs)*
            #[allow(unsafe_code)]
            fn #checked(#(#wrapper_args),*) #ret {
                // SAFETY: the signature was checked against the header above,
                // and every precondition declared on this wrapper has already
                // been checked by `#[contract]` when that predicate applies.
                unsafe { #raw(#(#call_args),*) }
            }
        };
        let inner = if has_contract {
            super::contract::expand(TokenStream::new(), inner)?
        } else {
            inner
        };
        let (unsafe_kw, safety_doc) = if has_pointer {
            (
                quote!(unsafe),
                quote!(#[doc = "# Safety\n\nEvery pointer argument must be valid for the C function it is passed to."]),
            )
        } else {
            (TokenStream::new(), TokenStream::new())
        };
        quote! {
            #inner
            #(#doc_attrs)*
            #safety_doc
            #vis #unsafe_kw fn #name(#(#wrapper_args),*) #ret {
                #checked(#(#call_args),*)
            }
        }
    };

    let link = LitStr::new(&c_name, sig.ident.span());
    Ok(quote! {
        extern "C" {
            #[link_name = #link]
            fn #raw(#(#raw_params),*) #ret;
        }
        #wrapper
    })
}

/// Finds a function declaration by name in bindgen's foreign blocks.
#[cfg(feature = "ffi-bindgen")]
fn find_c_function<'a>(ast: &'a syn::File, name: &str) -> Option<&'a syn::Signature> {
    ast.items.iter().find_map(|item| match item {
        Item::ForeignMod(fm) => fm.items.iter().find_map(|fi| match fi {
            ForeignItem::Fn(f) if f.sig.ident == name => Some(&f.sig),
            _ => None,
        }),
        _ => None,
    })
}

/// Compares the Rust declaration with the C one, type by type.
#[cfg(feature = "ffi-bindgen")]
fn check_signature(rust: &syn::Signature, c: &syn::Signature) -> syn::Result<()> {
    let name = &rust.ident;
    if rust.inputs.len() != c.inputs.len() {
        return Err(syn::Error::new(
            name.span(),
            format!(
                "`{name}` takes {} parameters in Rust but {} in the header",
                rust.inputs.len(),
                c.inputs.len()
            ),
        ));
    }
    for (i, (r, c_arg)) in rust.inputs.iter().zip(&c.inputs).enumerate() {
        let (FnArg::Typed(r), FnArg::Typed(cty)) = (r, c_arg) else {
            return Err(syn::Error::new(r.span(), "`self` is not supported"));
        };
        let rust_ty = canonical(&r.ty)?;
        let c_ty = canonical(&cty.ty)?;
        if rust_ty != c_ty {
            return Err(syn::Error::new(
                r.ty.span(),
                format!(
                    "parameter {i} of `{name}` is `{rust_ty}` in Rust but `{c_ty}` in the header"
                ),
            ));
        }
    }
    let rust_ret = return_canonical(&rust.output)?;
    let c_ret = return_canonical(&c.output)?;
    if rust_ret != c_ret {
        return Err(syn::Error::new(
            rust.output.span(),
            format!("`{name}` returns `{rust_ret}` in Rust but `{c_ret}` in the header"),
        ));
    }
    Ok(())
}

#[cfg(feature = "ffi-bindgen")]
fn return_canonical(ret: &ReturnType) -> syn::Result<String> {
    match ret {
        ReturnType::Default => Ok("()".to_string()),
        ReturnType::Type(_, ty) => canonical(ty),
    }
}

/// A string form of a type that is equal for two types exactly when they
/// denote the same C-compatible type. The `c_*` aliases map to the Rust
/// primitives they stand for.
#[cfg(feature = "ffi-bindgen")]
fn canonical(ty: &Type) -> syn::Result<String> {
    let unsupported = || {
        syn::Error::new(
            ty.span(),
            "unsupported type in an `ffi_wrappers!` signature: use a scalar, a raw pointer, or `c_void`",
        )
    };
    match ty {
        Type::Path(p) if p.qself.is_none() => {
            let seg = p.path.segments.last().ok_or_else(unsupported)?;
            if !seg.arguments.is_empty() {
                return Err(unsupported());
            }
            let name = seg.ident.to_string();
            let mapped = match name.as_str() {
                "c_char" | "c_schar" => "i8",
                "c_uchar" => "u8",
                "c_short" => "i16",
                "c_ushort" => "u16",
                "c_int" => "i32",
                "c_uint" => "u32",
                "c_longlong" => "i64",
                "c_ulonglong" => "u64",
                "c_float" => "f32",
                "c_double" => "f64",
                other => other,
            };
            Ok(mapped.to_string())
        }
        Type::Ptr(p) => {
            let mutability = if p.mutability.is_some() {
                "mut"
            } else {
                "const"
            };
            Ok(format!("*{mutability} {}", canonical(&p.elem)?))
        }
        Type::Tuple(t) if t.elems.is_empty() => Ok("()".to_string()),
        _ => Err(unsupported()),
    }
}

#[cfg(all(test, feature = "ffi-bindgen"))]
mod tests {
    use super::*;

    const HEADER: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tpt-reficon/tests/ffi/payment.h"
    );

    fn expand_str(decl: &str) -> syn::Result<String> {
        let src = format!(
            r#"header = "{}"; bridge = "c"; {decl}"#,
            HEADER.replace('\\', "/")
        );
        let input: TokenStream = src.parse().unwrap();
        expand(input).map(|ts| ts.to_string())
    }

    fn err(decl: &str) -> String {
        match expand_str(decl) {
            Ok(_) => panic!("expected an error for `{decl}`"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn matching_signature_expands_to_a_wrapper_and_a_raw_declaration() {
        let out = expand_str("pub fn process_payment(amount: i32) -> i32;").unwrap();
        assert!(out.contains("link_name"), "{out}");
        assert!(out.contains("__tpt_raw_process_payment"), "{out}");
        assert!(out.contains("allow (unsafe_code)"), "{out}");
    }

    #[test]
    fn contracts_on_the_wrapper_are_expanded() {
        let out = expand_str("#[requires(amount > 0)] pub fn process_payment(amount: i32) -> i32;")
            .unwrap();
        assert!(out.contains("precondition"), "{out}");
    }

    #[test]
    fn mismatched_parameter_type_is_rejected() {
        let msg = err("pub fn process_payment(amount: i64) -> i32;");
        assert!(
            msg.contains(
                "parameter 0 of `process_payment` is `i64` in Rust but `i32` in the header"
            ),
            "{msg}"
        );
    }

    #[test]
    fn mismatched_return_type_is_rejected() {
        let msg = err("pub fn process_payment(amount: i32) -> u32;");
        assert!(
            msg.contains("returns `u32` in Rust but `i32` in the header"),
            "{msg}"
        );
    }

    #[test]
    fn mismatched_parameter_count_is_rejected() {
        let msg = err("pub fn process_payment(amount: i32, extra: i32) -> i32;");
        assert!(
            msg.contains("takes 2 parameters in Rust but 1 in the header"),
            "{msg}"
        );
    }

    #[test]
    fn missing_function_is_rejected() {
        let msg = err("pub fn no_such_function(amount: i32) -> i32;");
        assert!(
            msg.contains("`no_such_function` is not declared in the header"),
            "{msg}"
        );
    }

    #[test]
    fn unknown_key_is_rejected() {
        let src = format!(
            r#"header = "{}"; flavour = "c";"#,
            HEADER.replace('\\', "/")
        );
        let input: TokenStream = src.parse().unwrap();
        let msg = expand(input).unwrap_err().to_string();
        assert!(
            msg.contains("unknown `ffi_wrappers` key `flavour`"),
            "{msg}"
        );
    }

    /// Expands a declaration against `buffer.h`, whose `sum_values` takes a
    /// raw pointer.
    fn expand_buffer(decl: &str) -> syn::Result<String> {
        let header = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tpt-reficon/tests/ffi/buffer.h"
        );
        let src = format!(
            r#"header = "{}"; bridge = "c"; {decl}"#,
            header.replace('\\', "/")
        );
        let input: TokenStream = src.parse().unwrap();
        expand(input).map(|ts| ts.to_string())
    }

    #[test]
    fn wrapper_taking_a_raw_pointer_is_unsafe() {
        // A safe `pub fn` here would let any caller pass a dangling pointer.
        let out =
            expand_buffer("pub fn sum_values(values: *const i32, count: u32) -> i32;").unwrap();
        assert!(out.contains("unsafe fn sum_values"), "{out}");
        assert!(out.contains("# Safety"), "{out}");
    }

    #[test]
    fn wrapper_without_pointers_stays_safe() {
        let out = expand_str("pub fn process_payment(amount: i32) -> i32;").unwrap();
        assert!(!out.contains("unsafe fn process_payment"), "{out}");
    }

    #[test]
    fn contract_on_a_pointer_wrapper_is_still_checked() {
        let out = expand_buffer(
            "#[requires(count > 0)] pub fn sum_values(values: *const i32, count: u32) -> i32;",
        )
        .unwrap();
        assert!(out.contains("unsafe fn sum_values"), "{out}");
        assert!(out.contains("precondition"), "{out}");
    }
}
