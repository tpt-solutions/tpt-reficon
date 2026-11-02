//! `#[ffi_contract(bridge = "...", header = "...")]`: proves that a `repr(C)`
//! struct mirrors the byte layout of a C struct declared in a header.
//!
//! The foreign layout comes from bindgen. With `layout_tests(true)`, bindgen
//! emits assertions whose constants are clang's own `sizeof`, `alignof`, and
//! `offsetof` values for every struct. We read those constants back out of the
//! generated AST, so the foreign side is what the C compiler computed rather
//! than a second model of the C layout rules.
//!
//! The Rust side is then checked with
//! [`tpt_reficon_verifier::layout::prove_equivalent`]. Two further assertions
//! are emitted into the user's crate: `size_of` and `align_of` must match the
//! foreign values. They are plain `const` checks, so they hold on whichever
//! target the crate is actually compiled for.
//!
//! Limits, enforced with compile errors rather than guessed around:
//! - only scalar fields, pointers, and arrays of those are supported;
//! - bitfields, padding fields, and unions are rejected;
//! - pointer width comes from the proc macro's host, which is checked by the
//!   `size_of` assertion when cross-compiling.
//!
//! `bridge = "go"` takes the same path: a Go struct only crosses the boundary
//! when it is declared as a C struct in the cgo preamble. See
//! `docs/design/go-header-ingestion.md`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, ExprLit, Fields, Ident, ItemStruct, Lit, LitStr, Meta, Token, Type};

#[cfg(feature = "ffi-bindgen")]
use syn::{BinOp, ExprBinary, ExprIndex, Item, Stmt};

use tpt_reficon_verifier::layout::{
    prove_equivalent, prove_within_bounds, Alignment, LayoutField, LayoutMismatch, StructLayout,
};

/// One `key = "value"` pair in the attribute arguments.
struct Arg {
    key: Ident,
    value: LitStr,
}

impl syn::parse::Parse for Arg {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let value: LitStr = input.parse()?;
        Ok(Arg { key, value })
    }
}

/// The parsed `#[ffi_contract(...)]` arguments.
struct Config {
    header: LitStr,
    c_name: Option<LitStr>,
}

fn parse_config(attr: TokenStream) -> syn::Result<Config> {
    let args: Punctuated<Arg, Token![,]> =
        syn::parse::Parser::parse2(Punctuated::<Arg, Token![,]>::parse_terminated, attr)?;
    let mut bridge = None;
    let mut header = None;
    let mut c_name = None;
    for arg in args {
        let slot = match arg.key.to_string().as_str() {
            "bridge" => &mut bridge,
            "header" => &mut header,
            "c_name" => &mut c_name,
            other => {
                return Err(syn::Error::new(
                    arg.key.span(),
                    format!("unknown `ffi_contract` key `{other}`; expected `bridge`, `header`, or `c_name`"),
                ))
            }
        };
        if slot.replace(arg.value).is_some() {
            return Err(syn::Error::new(
                arg.key.span(),
                "duplicate `ffi_contract` key",
            ));
        }
    }
    let bridge = bridge.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "`ffi_contract` requires `bridge = \"c\"` or `bridge = \"go\"`",
        )
    })?;
    match bridge.value().as_str() {
        "c" | "go" => {}
        _ => {
            return Err(syn::Error::new(
                bridge.span(),
                "`ffi_contract` bridge must be \"c\" or \"go\"",
            ))
        }
    }
    let header = header.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "`ffi_contract` requires `header = \"...\"`",
        )
    })?;
    Ok(Config { header, c_name })
}

/// Expands `#[ffi_contract(...)]` on a `#[repr(C)]` struct.
pub fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let config = parse_config(attr)?;
    let item: ItemStruct = syn::parse2(item)?;
    check_repr_c(&item)?;

    let c_name = config
        .c_name
        .as_ref()
        .map_or_else(|| item.ident.to_string(), LitStr::value);

    let foreign = read_foreign_layout(&config, &c_name)?;
    let rust_fields = rust_layout_fields(&item)?;
    check_against_foreign(&item, &rust_fields, &foreign)?;

    let name = &item.ident;
    let size = foreign.size as usize;
    let align = foreign.align.bytes() as usize;
    Ok(quote! {
        #item

        const _: () = ::core::assert!(
            ::core::mem::size_of::<#name>() == #size,
            "tpt-reficon: size of Rust struct differs from the C header"
        );
        const _: () = ::core::assert!(
            ::core::mem::align_of::<#name>() == #align,
            "tpt-reficon: alignment of Rust struct differs from the C header"
        );
    })
}

fn check_repr_c(item: &ItemStruct) -> syn::Result<()> {
    for attr in &item.attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }
        let metas = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        if metas
            .iter()
            .any(|m| matches!(m, Meta::Path(p) if p.is_ident("C")))
        {
            return Ok(());
        }
    }
    Err(syn::Error::new(
        item.ident.span(),
        "`#[ffi_contract]` requires `#[repr(C)]` on the struct",
    ))
}

/// Runs bindgen over the header and parses its output. With `layout_tests`,
/// the output also carries clang's layout assertions.
#[cfg(feature = "ffi-bindgen")]
pub(crate) fn bindgen_ast(header: &LitStr, layout_tests: bool) -> syn::Result<syn::File> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| syn::Error::new(header.span(), "CARGO_MANIFEST_DIR is not set"))?;
    let header_path = std::path::Path::new(&manifest).join(header.value());
    let header_str = header_path.to_string_lossy().into_owned();

    let bindings = bindgen::Builder::default()
        .header(header_str)
        .layout_tests(layout_tests)
        .generate()
        .map_err(|e| {
            syn::Error::new(
                header.span(),
                format!("failed to parse C header `{}`: {e}", header.value()),
            )
        })?;
    syn::parse_str(&bindings.to_string())
        .map_err(|e| syn::Error::new(header.span(), format!("unexpected bindgen output: {e}")))
}

/// Reads the foreign struct's layout from the bindgen output for `header`.
#[cfg(feature = "ffi-bindgen")]
fn read_foreign_layout(config: &Config, c_name: &str) -> syn::Result<StructLayout> {
    let generated = bindgen_ast(&config.header, true)?;

    let s = find_struct(&generated, c_name).ok_or_else(|| {
        syn::Error::new(
            config.header.span(),
            format!(
                "struct `{c_name}` not found in `{}`; check `c_name` or the header",
                config.header.value()
            ),
        )
    })?;
    let (size, align, offsets) = read_layout_asserts(&generated, c_name, config.header.span())?;

    let mut fields = Vec::new();
    let mut field_offsets = Vec::new();
    for field in &s.fields {
        let ident = match &field.ident {
            Some(i) => i.to_string(),
            None => {
                return Err(syn::Error::new(
                    config.header.span(),
                    format!("`{c_name}` has unnamed fields, which are not supported"),
                ))
            }
        };
        if ident.starts_with("_bitfield") || ident.starts_with("__bindgen_padding") {
            return Err(syn::Error::new(
                config.header.span(),
                format!("`{c_name}` uses bitfields or padding, which `#[ffi_contract]` does not support"),
            ));
        }
        let (fsize, falign) = type_layout(&field.ty).ok_or_else(|| {
            syn::Error::new(
                config.header.span(),
                format!("unsupported C field type for `{c_name}::{ident}`"),
            )
        })?;
        let offset = *offsets.get(&ident).ok_or_else(|| {
            syn::Error::new(
                config.header.span(),
                format!("no clang offset recorded for `{c_name}::{ident}`"),
            )
        })?;
        fields.push(LayoutField {
            name: ident,
            size: fsize,
            align: falign,
        });
        field_offsets.push(offset);
    }

    Ok(StructLayout {
        fields,
        offsets: field_offsets,
        size,
        align,
    })
}

#[cfg(not(feature = "ffi-bindgen"))]
fn read_foreign_layout(config: &Config, _c_name: &str) -> syn::Result<StructLayout> {
    Err(syn::Error::new(
        config.header.span(),
        "`#[ffi_contract]` needs the `ffi-bindgen` feature of `tpt-reficon-macros` (or `tpt-reficon/ffi`)",
    ))
}

#[cfg(feature = "ffi-bindgen")]
fn find_struct<'a>(file: &'a syn::File, c_name: &str) -> Option<&'a syn::ItemStruct> {
    file.items.iter().find_map(|item| match item {
        Item::Struct(s) if s.ident == c_name => Some(s),
        _ => None,
    })
}

/// Pulls `(size, align, field offsets)` out of bindgen's layout assertions.
///
/// bindgen writes them as `const _: () = { ... };` blocks, one statement per
/// fact, shaped `["<message>"][<expr> - <clang value>usize];`. The clang value
/// is the literal on the right of the subtraction. Messages are matched
/// exactly against the C struct's name, so other structs' blocks are ignored.
#[cfg(feature = "ffi-bindgen")]
fn read_layout_asserts(
    file: &syn::File,
    c_name: &str,
    span: proc_macro2::Span,
) -> syn::Result<(u64, Alignment, std::collections::BTreeMap<String, u64>)> {
    let blocks = file.items.iter().filter_map(|item| match item {
        Item::Const(c) => match c.expr.as_ref() {
            Expr::Block(b) => Some(&b.block),
            _ => None,
        },
        _ => None,
    });

    let mut size = None;
    let mut align = None;
    let mut offsets = std::collections::BTreeMap::new();
    let size_msg = format!("Size of {c_name}");
    let align_msg = format!("Alignment of {c_name}");
    let offset_prefix = format!("Offset of field: {c_name}::");

    for stmt in blocks.flat_map(|b| b.stmts.iter()) {
        let Stmt::Expr(Expr::Index(ExprIndex { expr, index, .. }), _) = stmt else {
            continue;
        };
        let Some(message) = assertion_message(expr) else {
            continue;
        };
        let value = assertion_value(index).ok_or_else(|| {
            syn::Error::new(span, format!("unrecognised layout assertion `{message}`"))
        })?;
        if message == size_msg {
            size = Some(value);
        } else if message == align_msg {
            align = Some(value);
        } else if let Some(field) = message.strip_prefix(&offset_prefix) {
            offsets.insert(field.to_string(), value);
        }
    }

    let size = size.ok_or_else(|| syn::Error::new(span, "no clang size recorded"))?;
    let align_value = align.ok_or_else(|| syn::Error::new(span, "no clang alignment recorded"))?;
    let align = u32::try_from(align_value)
        .ok()
        .and_then(Alignment::new)
        .ok_or_else(|| syn::Error::new(span, "clang alignment is not a power of two"))?;
    Ok((size, align, offsets))
}

/// The string message of an assertion array `["message"]`.
#[cfg(feature = "ffi-bindgen")]
fn assertion_message(expr: &Expr) -> Option<String> {
    let Expr::Array(arr) = expr else {
        return None;
    };
    match arr.elems.first()? {
        Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) if arr.elems.len() == 1 => Some(s.value()),
        _ => None,
    }
}

/// The clang value in `<expr> - <literal>`.
#[cfg(feature = "ffi-bindgen")]
fn assertion_value(index: &Expr) -> Option<u64> {
    let Expr::Binary(ExprBinary {
        op: BinOp::Sub(_),
        right,
        ..
    }) = index
    else {
        return None;
    };
    match right.as_ref() {
        Expr::Lit(ExprLit {
            lit: Lit::Int(i), ..
        }) => i.base10_parse::<u64>().ok(),
        _ => None,
    }
}

/// `(size, align)` for a field type, or `None` if it is outside the supported
/// subset. Used for both the foreign and Rust sides, so the two agree by
/// construction on what each type means.
fn type_layout(ty: &Type) -> Option<(u64, Alignment)> {
    match ty {
        Type::Path(p) if p.qself.is_none() => {
            let ident = p.path.segments.last()?.ident.to_string();
            scalar_layout(&ident)
        }
        Type::Ptr(_) => {
            let w = std::mem::size_of::<usize>() as u64;
            Some((w, Alignment::new(w as u32)?))
        }
        Type::Array(arr) => {
            let len = match &arr.len {
                Expr::Lit(ExprLit {
                    lit: Lit::Int(i), ..
                }) => i.base10_parse::<u64>().ok()?,
                _ => return None,
            };
            let (elem_size, elem_align) = type_layout(&arr.elem)?;
            Some((elem_size.checked_mul(len)?, elem_align))
        }
        _ => None,
    }
}

fn scalar_layout(name: &str) -> Option<(u64, Alignment)> {
    let bytes: u64 = match name {
        "u8" | "i8" | "bool" | "c_char" | "c_uchar" | "c_schar" => 1,
        "u16" | "i16" | "c_short" | "c_ushort" => 2,
        "u32" | "i32" | "f32" | "c_int" | "c_uint" | "c_float" => 4,
        "u64" | "i64" | "f64" | "c_longlong" | "c_ulonglong" | "c_double" => 8,
        "usize" | "isize" => std::mem::size_of::<usize>() as u64,
        _ => return None,
    };
    Some((bytes, Alignment::new(bytes as u32)?))
}

fn rust_layout_fields(item: &ItemStruct) -> syn::Result<Vec<LayoutField>> {
    let Fields::Named(named) = &item.fields else {
        return Err(syn::Error::new(
            item.span(),
            "`#[ffi_contract]` supports only structs with named fields",
        ));
    };
    named
        .named
        .iter()
        .map(|f| {
            let ident = f.ident.as_ref().expect("named fields have identifiers");
            let (size, align) = type_layout(&f.ty).ok_or_else(|| {
                syn::Error::new(
                    f.ty.span(),
                    "unsupported field type for `#[ffi_contract]`: use a scalar, a raw pointer, or an array of those",
                )
            })?;
            Ok(LayoutField {
                name: ident.to_string().trim_start_matches("r#").to_string(),
                size,
                align,
            })
        })
        .collect()
}

fn check_against_foreign(
    item: &ItemStruct,
    rust_fields: &[LayoutField],
    foreign: &StructLayout,
) -> syn::Result<()> {
    let rust_layout = StructLayout::compute(rust_fields)
        .ok_or_else(|| syn::Error::new(item.ident.span(), "Rust struct layout overflows"))?;
    prove_within_bounds(&rust_layout)
        .map_err(|m| syn::Error::new(item.ident.span(), describe(&m)))?;
    prove_equivalent(rust_fields, foreign)
        .map_err(|m| syn::Error::new(item.ident.span(), describe(&m)))
}

/// Human-readable form of a layout mismatch for the compile error.
fn describe(m: &LayoutMismatch) -> String {
    match m {
        LayoutMismatch::FieldCountMismatch { rust, foreign } => {
            format!("field count differs: Rust struct has {rust}, C header has {foreign}")
        }
        LayoutMismatch::FieldNameMismatch {
            index,
            rust,
            foreign,
        } => format!("field {index} is named `{rust}` in Rust but `{foreign}` in C"),
        LayoutMismatch::OffsetMismatch {
            index,
            rust,
            foreign,
        } => format!("field {index} is at offset {rust} in Rust but {foreign} in C"),
        LayoutMismatch::FieldSizeMismatch {
            index,
            rust,
            foreign,
        } => format!("field {index} is {rust} bytes in Rust but {foreign} bytes in C"),
        LayoutMismatch::AlignmentMismatch { rust, foreign } => {
            format!("alignment is {rust} in Rust but {foreign} in C")
        }
        LayoutMismatch::StructSizeMismatch { rust, foreign } => {
            format!("struct size is {rust} bytes in Rust but {foreign} bytes in C")
        }
        other => format!("layout could not be proven equivalent: {other:?}"),
    }
}

#[cfg(all(test, feature = "ffi-bindgen"))]
mod tests {
    use super::*;
    use proptest::prelude::prop_assert;
    use proptest::prop_assume;

    const HEADER: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tpt-reficon/tests/ffi/payment.h"
    );

    fn expand_with(attr: &str, item: &str) -> syn::Result<TokenStream> {
        let attr: TokenStream = attr.parse().unwrap();
        let item: TokenStream = item.parse().unwrap();
        expand(attr, item)
    }

    fn err_text(r: syn::Result<TokenStream>) -> String {
        match r {
            Ok(_) => panic!("expected an error"),
            Err(e) => e.to_string(),
        }
    }

    /// Forward slashes keep the path valid inside a string literal on Windows.
    fn header() -> String {
        HEADER.replace('\\', "/")
    }

    fn header_attr() -> String {
        format!(
            r#"bridge = "c", header = "{}", c_name = "payment""#,
            header()
        )
    }

    #[test]
    fn matching_struct_expands_with_size_and_align_asserts() {
        let out = expand_with(
            &header_attr(),
            "#[repr(C)] pub struct Payment { id: u64, amount: i32, flag: u8, ptr: *mut f64 }",
        )
        .unwrap()
        .to_string();
        assert!(out.contains("size_of"), "{out}");
        assert!(out.contains("align_of"), "{out}");
    }

    #[test]
    fn reordered_fields_are_rejected() {
        let msg = err_text(expand_with(
            &header_attr(),
            "#[repr(C)] struct Payment { amount: i32, id: u64, flag: u8, ptr: *mut f64 }",
        ));
        assert!(msg.contains("field 0 is named `amount`"), "{msg}");
    }

    #[test]
    fn wrong_scalar_width_is_rejected() {
        let msg = err_text(expand_with(
            &header_attr(),
            "#[repr(C)] struct Payment { id: u64, amount: i64, flag: u8, ptr: *mut f64 }",
        ));
        assert!(
            msg.contains("field 1 is 8 bytes in Rust but 4 bytes in C"),
            "{msg}"
        );
    }

    #[test]
    fn missing_field_is_rejected() {
        let msg = err_text(expand_with(
            &header_attr(),
            "#[repr(C)] struct Payment { id: u64, amount: i32, flag: u8 }",
        ));
        assert!(msg.contains("field count differs"), "{msg}");
    }

    #[test]
    fn missing_repr_c_is_rejected() {
        let msg = err_text(expand_with(
            &header_attr(),
            "struct Payment { id: u64, amount: i32, flag: u8, ptr: *mut f64 }",
        ));
        assert!(msg.contains("requires `#[repr(C)]`"), "{msg}");
    }

    #[test]
    fn unknown_struct_name_is_rejected() {
        let msg = err_text(expand_with(
            &format!(r#"bridge = "c", header = "{}""#, header()),
            "#[repr(C)] struct Nope { id: u64 }",
        ));
        assert!(msg.contains("struct `Nope` not found"), "{msg}");
    }

    #[test]
    fn unsupported_field_type_is_rejected() {
        let msg = err_text(expand_with(
            &header_attr(),
            "#[repr(C)] struct Payment { id: u64, amount: i32, flag: u8, ptr: String }",
        ));
        assert!(msg.contains("unsupported field type"), "{msg}");
    }

    proptest::proptest! {
        /// Arbitrary attribute and item text must produce `Ok` or a `syn::Error`,
        /// never a panic. Attribute text is untrusted input to the frontend.
        #[test]
        fn expand_never_panics_on_arbitrary_input(
            attr in "[ -~]{0,80}",
            item in "[ -~]{0,120}",
        ) {
            if let (Ok(attr), Ok(item)) = (attr.parse::<TokenStream>(), item.parse::<TokenStream>()) {
                let _ = expand(attr, item);
            }
        }

        /// Attribute keys other than the known set are always rejected cleanly.
        #[test]
        fn unknown_keys_are_rejected(key in "[a-z_]{1,12}") {
            prop_assume!(!matches!(key.as_str(), "bridge" | "header" | "c_name"));
            let attr: TokenStream = format!(r#"{key} = "x""#).parse().unwrap();
            prop_assert!(parse_config(attr).is_err());
        }
    }

    #[test]
    fn bad_bridge_is_rejected() {
        let msg = err_text(expand_with(
            r#"bridge = "rust", header = "x.h""#,
            "#[repr(C)] struct Payment { id: u64 }",
        ));
        assert!(msg.contains("bridge must be"), "{msg}");
    }
}
