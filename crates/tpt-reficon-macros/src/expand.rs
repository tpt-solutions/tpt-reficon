//! The `#[derive(Refine)]` expansion.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Expr, Fields, Ident};

use crate::{attrs, domain, integer, path, predicate};

pub fn expand_refine(input: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let (struct_span, field_access, field_constructor, field_def_ident, inner_ty) =
        check_newtype_struct(input)?;

    if input.generics.params.iter().next().is_some() {
        return Err(syn::Error::new(
            input.generics.span(),
            "derive(Refine) does not support generic structs yet",
        ));
    }

    // Aliases and qualified paths are not resolved, so their domain and their
    // integer widening are unknown. Reject them rather than guess.
    if domain::primitive_name(inner_ty).is_none() {
        return Err(syn::Error::new(
            inner_ty.span(),
            format!(
                "derive(Refine) needs a primitive inner type (`u8`, `i64`, `bool`, `f64`, ...); \
                 `{}` is not one, and type aliases and paths are not resolved",
                quote!(#inner_ty)
            ),
        ));
    }

    let config = attrs::parse_config(&input.attrs, field_def_ident)?;
    // `Ident::new` panics on a string that is not an identifier, so parse it and
    // report a compile error instead.
    let variable = syn::parse_str::<Ident>(&config.variable).map_err(|_| {
        syn::Error::new(
            struct_span,
            format!("`{}` is not a valid variable name", config.variable),
        )
    })?;
    let allowed = ::core::slice::from_ref(&config.variable);

    if config.predicates.iter().any(predicate::uses_remainder) && !domain::is_unsigned(inner_ty) {
        return Err(syn::Error::new(
            inner_ty.span(),
            "`%` in a refinement needs an unsigned inner type, because a signed remainder takes the sign of the dividend",
        ));
    }

    // 1. Lowers every predicate to DNF over the verifier's constraint IR.
    //    Multiple attributes are conjoined; `||` within one predicate
    //    distributes into alternatives.
    let mut forms = Vec::with_capacity(config.predicates.len());
    let mut render = String::new();
    for expr in &config.predicates {
        forms.push(predicate::lower(expr, allowed)?);
        if !render.is_empty() {
            render.push_str(" && ");
        }
        render.push_str(&quote!(#expr).to_string());
    }
    let render = if render.is_empty() {
        "true".to_string()
    } else {
        render
    };
    let dnf = predicate::conjoin(&forms).ok_or_else(|| {
        syn::Error::new(
            struct_span,
            format!(
                "refinement type `{name}` has too many predicate alternatives (`{render}`); \
                 simplify the disjunctions"
            ),
        )
    })?;

    let inner_render = quote!(#inner_ty).to_string();
    let unsatisfiable = || {
        syn::Error::new(
            struct_span,
            format!(
                "unsatisfiable refinement type `{name}`: no value of `{inner_render}` satisfies `{render}`"
            ),
        )
    };

    // 2. An unsatisfiable refinement can never be constructed — compile error.
    //    A disjunction is unsatisfiable only when every alternative is.
    if predicate::is_unsat(&dnf) {
        return Err(unsatisfiable());
    }

    // 3. Zero-cost elision: the type domain proving some alternative means
    //    `new`/`try_new` never need a runtime check.
    let domain = domain::domain_constraints(inner_ty, &config.variable);

    // 4. Integer-domain completeness: the solver decides over ℚ, but for
    //    primitive integer types a rational-only witness (e.g. the gap
    //    `value > 5 && value < 6`, or `value > 255` on `u8`) is not
    //    constructible. Reject a refinement only when *every* alternative is
    //    proven to admit no integer in the type's domain.
    if domain::is_supported_integer(inner_ty)
        && dnf.iter().all(|branch| {
            let mut combined = domain.clone();
            combined.extend(branch.iter().cloned());
            matches!(
                integer::analyze(&combined, &config.variable),
                integer::IntegerFeasibility::Infeasible
            )
        })
    {
        return Err(unsatisfiable());
    }

    let elided = predicate::entailed_by(&domain, &dnf);

    // 4. Runtime predicate: the conjoined original expressions, evaluated
    //    with the refined variable bound to the value under construction.
    let runtime_check: Expr = match config.predicates.as_slice() {
        [] => syn::parse_quote!(true),
        preds if preds.len() == 1 => preds[0].clone(),
        _ => {
            let preds = &config.predicates;
            // Each predicate is parenthesized: `a && b || c` would otherwise
            // parse as `(a && b) || c` and accept values that violate `a`.
            syn::parse_quote!((#((#preds))&&*))
        }
    };

    let core = path::core_crate();

    // Precompute the diagnostic text at expansion time so the generated code
    // carries a `'static` message and never needs to format at runtime.
    let message = format!("refinement `{render}` must hold for `{name}`");
    let msg_literal = syn::LitStr::new(&message, Span::call_site());
    let parse_msg = syn::LitStr::new(
        &format!("input does not parse as `{inner_render}`"),
        Span::call_site(),
    );

    // Integer inner types evaluate the runtime predicate in `i128`: the solver
    // reasons over ℚ/i64, and evaluating in the narrow inner type (e.g. u8)
    // would panic in debug / wrap in release on overflow — a guarantee the
    // derive must not ship. Non-integer types keep their native arithmetic.
    let widen_to_i128 = domain::is_supported_integer(inner_ty);

    let (new_fn, try_new_fn) = if elided {
        (
            quote! {
                #[allow(dead_code)]
                #[doc = "Constructs a refined value without a runtime check: the type's domain \
                         already implies the refinement, so this was proven at compile time."]
                #[track_caller]
                pub fn new(value: #inner_ty) -> Self {
                    #field_constructor
                }
            },
            quote! {
                #[allow(dead_code)]
                #[doc = "Always `Ok` — the refinement was proven by the type's domain at compile time."]
                pub fn try_new(value: #inner_ty) -> ::core::result::Result<Self, #core::RefinementError> {
                    ::core::result::Result::Ok(#field_constructor)
                }
            },
        )
    } else if widen_to_i128 {
        // A value outside `i128` (only possible for `u128`) cannot be evaluated
        // in `i128`. A plain `as` cast would wrap it negative and could accept
        // a value that violates the predicate, so the labeled block below
        // rejects it instead. That refuses some valid `u128` values, a
        // conservative loss that never admits an invalid one.
        (
            quote! {
                #[allow(dead_code)]
                #[doc = "Constructs a refined value, panicking if it violates the refinement. \
                         Runtime check: the type's domain does not prove the predicate, so the check is kept."]
                #[track_caller]
                pub fn new(value: #inner_ty) -> Self {
                    if 'check: {
                        // The inner binding shadows the parameter inside the
                        // condition only; `value` remains the original inner
                        // value for the constructor.
                        let #variable: i128 = match ::core::convert::TryInto::<i128>::try_into(value) {
                            ::core::result::Result::Ok(v) => v,
                            ::core::result::Result::Err(_) => break 'check false,
                        };
                        #runtime_check
                    } {
                        #field_constructor
                    } else {
                        ::core::panic!("{}", #core::RefinementError::with_message(#msg_literal).with_value(value))
                    }
                }
            },
            quote! {
                #[allow(dead_code)]
                #[doc = "Constructs a refined value, returning an error on violation."]
                pub fn try_new(value: #inner_ty) -> ::core::result::Result<Self, #core::RefinementError> {
                    if 'check: {
                        let #variable: i128 = match ::core::convert::TryInto::<i128>::try_into(value) {
                            ::core::result::Result::Ok(v) => v,
                            ::core::result::Result::Err(_) => break 'check false,
                        };
                        #runtime_check
                    } {
                        ::core::result::Result::Ok(#field_constructor)
                    } else {
                        ::core::result::Result::Err(#core::RefinementError::with_message(#msg_literal).with_value(value))
                    }
                }
            },
        )
    } else {
        (
            quote! {
                #[allow(dead_code)]
                #[doc = "Constructs a refined value, panicking if it violates the refinement. \
                         Runtime check: the type's domain does not prove the predicate, so the check is kept."]
                #[track_caller]
                pub fn new(value: #inner_ty) -> Self {
                    let #variable: #inner_ty = value;
                    if #runtime_check {
                        #field_constructor
                    } else {
                        ::core::panic!("{}", #core::RefinementError::with_message(#msg_literal).with_value(value))
                    }
                }
            },
            quote! {
                #[allow(dead_code)]
                #[doc = "Constructs a refined value, returning an error on violation."]
                pub fn try_new(value: #inner_ty) -> ::core::result::Result<Self, #core::RefinementError> {
                    let #variable: #inner_ty = value;
                    if #runtime_check {
                        ::core::result::Result::Ok(#field_constructor)
                    } else {
                        ::core::result::Result::Err(#core::RefinementError::with_message(#msg_literal).with_value(value))
                    }
                }
            },
        )
    };

    // `#[refine(serde)]`: deserialize the inner type, then check the refinement,
    // so untrusted input cannot build an invalid value. The user crate must depend
    // on serde for this to compile, which is why it is opt-in.
    let serde_impl = if config.serde {
        quote! {
            impl<'de> ::serde::Deserialize<'de> for #name {
                fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
                where
                    D: ::serde::Deserializer<'de>,
                {
                    let value = <#inner_ty as ::serde::Deserialize<'de>>::deserialize(deserializer)?;
                    Self::try_new(value).map_err(<D::Error as ::serde::de::Error>::custom)
                }
            }
        }
    } else {
        TokenStream::new()
    };

    // `#[refine(proptest)]`: draws the inner type and keeps the values that pass
    // `try_new`, so a property test sees only valid values. A very narrow
    // refinement rejects most draws, which makes the test slow, not wrong.
    let proptest_impl = if config.proptest {
        quote! {
            impl ::proptest::arbitrary::Arbitrary for #name {
                type Parameters = ();
                type Strategy = ::proptest::strategy::BoxedStrategy<Self>;
                fn arbitrary_with(_: ()) -> Self::Strategy {
                    use ::proptest::strategy::Strategy;
                    ::proptest::arbitrary::any::<#inner_ty>()
                        .prop_filter_map("refinement", |v| <#name>::try_new(v).ok())
                        .boxed()
                }
            }
        }
    } else {
        TokenStream::new()
    };

    // The body of `__tpt_check`, mirroring the runtime check. Integers widen to
    // `i128` as in `new`. A `u128` above `i128::MAX` is rejected first, because the
    // `as` cast below would wrap it.
    let check_body = if widen_to_i128 {
        let u128_guard = if quote!(#inner_ty).to_string() == "u128" {
            quote! { if (value as u128) > (i128::MAX as u128) { return false; } }
        } else {
            TokenStream::new()
        };
        quote! {
            #u128_guard
            let #variable: i128 = value as i128;
            #runtime_check
        }
    } else {
        quote! {
            let #variable: #inner_ty = value;
            #runtime_check
        }
    };

    Ok(quote! {
        #serde_impl
        #proptest_impl

        #[automatically_derived]
        impl #core::Refine for #name {
            type Inner = #inner_ty;

            fn into_inner(self) -> #inner_ty {
                #field_access
            }
        }

        impl ::core::convert::TryFrom<#inner_ty> for #name {
            type Error = #core::RefinementError;
            fn try_from(value: #inner_ty) -> ::core::result::Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl ::core::ops::Deref for #name {
            type Target = #inner_ty;
            fn deref(&self) -> &Self::Target {
                &#field_access
            }
        }

        // The reverse direction is total, so it needs no check: a refined value
        // is always a valid inner value. The refined-to-plain conversion is the
        // one that is always safe to write as `.into()`.
        impl ::core::convert::From<#name> for #inner_ty {
            fn from(value: #name) -> Self {
                <#name as #core::Refine>::into_inner(value)
            }
        }

        impl ::core::convert::AsRef<#inner_ty> for #name {
            fn as_ref(&self) -> &#inner_ty {
                &#field_access
            }
        }

        // Displays the inner value, so a refined number prints as the number.
        impl ::core::fmt::Display for #name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&#field_access, f)
            }
        }

        // Parses the inner type, then checks the refinement, so "0" fails for a
        // non-zero type with the same error as `try_new`.
        impl ::core::str::FromStr for #name {
            type Err = #core::RefinementError;
            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                let value = <#inner_ty as ::core::str::FromStr>::from_str(s).map_err(|_| {
                    #core::RefinementError::with_message(#parse_msg).with_value(s)
                })?;
                Self::try_new(value)
            }
        }

        impl #name {
            #new_fn
            #try_new_fn

            #[allow(dead_code)]
            #[doc = "Returns the raw inner value."]
            pub fn get(&self) -> #inner_ty {
                #field_access
            }

            // The predicate as a `const fn`, so `refined!` can check a literal at
            // compile time. The body mirrors the runtime check, including the
            // widening to `i128` for integers.
            #[doc(hidden)]
            pub const fn __tpt_check(value: #inner_ty) -> bool {
                #check_body
            }
        }
    })
}

/// Validates the input is a single-field struct and derives:
/// (struct span, self-field access tokens, constructor tokens, the field's
/// identifier (None for tuple structs), the field's type).
fn check_newtype_struct(
    input: &DeriveInput,
) -> syn::Result<(Span, TokenStream, TokenStream, Option<&Ident>, &syn::Type)> {
    let fields = match &input.data {
        Data::Struct(s) => &s.fields,
        _ => {
            return Err(syn::Error::new(
                input.span(),
                "derive(Refine) is only supported on structs",
            ));
        }
    };
    let err = || {
        syn::Error::new(
            input.span(),
            "derive(Refine) expects exactly one field: a tuple struct like \
             `struct Bounded(u8)` or a named-field struct like `struct S { v: u8 }`",
        )
    };
    // A public inner field lets any caller write an unchecked value straight
    // into the struct, which bypasses every predicate. Only `Self`'s own
    // constructors may build one.
    let private = |field: &syn::Field| -> syn::Result<()> {
        if matches!(field.vis, syn::Visibility::Inherited) {
            Ok(())
        } else {
            Err(syn::Error::new(
                field.vis.span(),
                "derive(Refine) requires a private inner field: a `pub` field lets callers \
                 build the value without checking the refinement",
            ))
        }
    };
    match fields {
        Fields::Unnamed(f) if f.unnamed.len() == 1 => {
            let field = f.unnamed.first().unwrap();
            private(field)?;
            Ok((
                input.span(),
                quote!(self.0),
                quote!(Self(value)),
                None,
                &field.ty,
            ))
        }
        Fields::Named(f) if f.named.len() == 1 => {
            let field = f.named.first().unwrap();
            private(field)?;
            let ident = field.ident.as_ref().expect("named field has ident");
            let access = quote!(self.#ident);
            let ctor = quote!(Self { #ident: value });
            Ok((input.span(), access, ctor, Some(ident), &field.ty))
        }
        _ => Err(err()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand_str(src: &str) -> String {
        let input: DeriveInput = syn::parse_str(src).unwrap();
        expand_refine(&input)
            .unwrap_or_else(|e| panic!("expansion failed: {e}"))
            .to_string()
    }

    fn expand_err(src: &str) -> String {
        let input: DeriveInput = syn::parse_str(src).unwrap();
        match expand_refine(&input) {
            Ok(_) => panic!("expected an error"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn u128_runtime_check_does_not_wrap() {
        // The runtime check uses TryInto. The one `as i128` is in the const check,
        // and it sits behind the guard that rejects values above i128::MAX first.
        let out = expand_str("#[requires(value < 1000)] struct B(u128);");
        assert!(out.contains("TryInto"), "{out}");
        assert_eq!(out.matches("as i128").count(), 1, "{out}");
        assert!(out.contains("i128 :: MAX as u128"), "{out}");
    }

    #[test]
    fn public_inner_field_is_rejected() {
        let msg = expand_err("struct B(pub u8);");
        assert!(msg.contains("private inner field"), "{msg}");
        let msg = expand_err("struct B { pub v: u8 }");
        assert!(msg.contains("private inner field"), "{msg}");
    }

    #[test]
    fn type_alias_inner_type_is_rejected() {
        // `Byte` could be any type, so its domain and widening are unknown.
        let msg = expand_err("struct B(Byte);");
        assert!(msg.contains("primitive inner type"), "{msg}");
        let msg = expand_err("struct B(std::primitive::u8);");
        assert!(msg.contains("primitive inner type"), "{msg}");
    }

    #[test]
    fn derive_extras_are_generated() {
        let out = expand_str("struct B(u8);");
        assert!(out.contains("FromStr"), "{out}");
        assert!(out.contains("AsRef"), "{out}");
        assert!(out.contains("Display"), "{out}");
    }

    #[test]
    fn no_unchecked_constructor_is_generated() {
        let out = expand_str("struct B(u8);");
        assert!(!out.contains("from_inner_unchecked"), "{out}");
    }
}

#[cfg(test)]
mod panic_free_tests {
    use super::*;

    fn expand_err(src: &str) -> String {
        let input: DeriveInput = syn::parse_str(src).unwrap();
        match expand_refine(&input) {
            Ok(_) => panic!("expected an error"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn invalid_variable_name_is_an_error_not_a_panic() {
        let msg = expand_err("#[refine(variable = \"1x\")] #[requires(value < 5)] struct B(u8);");
        assert!(msg.contains("not a valid variable name"), "{msg}");
    }
}

#[cfg(test)]
mod serde_tests {
    use super::*;

    fn expand_str(src: &str) -> String {
        let input: DeriveInput = syn::parse_str(src).unwrap();
        expand_refine(&input).unwrap().to_string()
    }

    #[test]
    fn deserialize_is_generated_only_on_request() {
        assert!(!expand_str("struct B(u8);").contains("Deserialize"));
        let out = expand_str("#[refine(serde)] struct B(u8);");
        assert!(out.contains("Deserialize"), "{out}");
        assert!(out.contains("try_new"), "{out}");
    }

    #[test]
    fn unknown_refine_key_names_both_options() {
        let input: DeriveInput = syn::parse_str("#[refine(flavour)] struct B(u8);").unwrap();
        let msg = expand_refine(&input).unwrap_err().to_string();
        assert!(msg.contains("`proptest`"), "{msg}");
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;

    #[test]
    fn arbitrary_is_generated_only_on_request() {
        let plain: DeriveInput = syn::parse_str("struct B(u8);").unwrap();
        assert!(!expand_refine(&plain)
            .unwrap()
            .to_string()
            .contains("Arbitrary"));
        let opted: DeriveInput = syn::parse_str("#[refine(proptest)] struct B(u8);").unwrap();
        let out = expand_refine(&opted).unwrap().to_string();
        assert!(out.contains("Arbitrary"), "{out}");
        assert!(out.contains("prop_filter_map"), "{out}");
    }
}

#[cfg(test)]
mod remainder_tests {
    use super::*;

    #[test]
    fn remainder_over_a_signed_type_is_rejected() {
        let input: DeriveInput =
            syn::parse_str("#[requires(value % 2 == 0)] struct B(i32);").unwrap();
        let msg = expand_refine(&input).unwrap_err().to_string();
        assert!(msg.contains("unsigned inner type"), "{msg}");
    }

    #[test]
    fn remainder_over_an_unsigned_type_expands() {
        let input: DeriveInput =
            syn::parse_str("#[requires(value % 2 == 0)] struct B(u32);").unwrap();
        let out = expand_refine(&input).unwrap().to_string();
        assert!(out.contains("__tpt_check"), "{out}");
    }
}
