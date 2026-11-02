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

    let config = attrs::parse_config(&input.attrs, field_def_ident)?;
    let variable = Ident::new(&config.variable, Span::call_site());
    let allowed = ::core::slice::from_ref(&config.variable);

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
        (
            quote! {
                #[allow(dead_code)]
                #[doc = "Constructs a refined value, panicking if it violates the refinement. \
                         The check is elided only where the verifier proved it redundant."]
                pub fn new(value: #inner_ty) -> Self {
                    if {
                        // The inner binding shadows the parameter inside the
                        // condition only; `value` remains the original inner
                        // value for the constructor.
                        let #variable: i128 = value as i128;
                        #runtime_check
                    } {
                        #field_constructor
                    } else {
                        ::core::panic!("{}", #msg_literal)
                    }
                }
            },
            quote! {
                #[allow(dead_code)]
                #[doc = "Constructs a refined value, returning an error on violation."]
                pub fn try_new(value: #inner_ty) -> ::core::result::Result<Self, #core::RefinementError> {
                    if {
                        let #variable: i128 = value as i128;
                        #runtime_check
                    } {
                        ::core::result::Result::Ok(#field_constructor)
                    } else {
                        ::core::result::Result::Err(#core::RefinementError::with_message(#msg_literal))
                    }
                }
            },
        )
    } else {
        (
            quote! {
                #[allow(dead_code)]
                #[doc = "Constructs a refined value, panicking if it violates the refinement. \
                         The check is elided only where the verifier proved it redundant."]
                pub fn new(value: #inner_ty) -> Self {
                    let #variable: #inner_ty = value;
                    if #runtime_check {
                        #field_constructor
                    } else {
                        ::core::panic!("{}", #msg_literal)
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
                        ::core::result::Result::Err(#core::RefinementError::with_message(#msg_literal))
                    }
                }
            },
        )
    };

    Ok(quote! {
        #[automatically_derived]
        impl #core::Refine for #name {
            type Inner = #inner_ty;

            fn from_inner_unchecked(value: #inner_ty) -> Self {
                #field_constructor
            }

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

        impl #name {
            #new_fn
            #try_new_fn

            #[allow(dead_code)]
            #[doc = "Returns the raw inner value."]
            pub fn get(&self) -> #inner_ty {
                #field_access
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
    match fields {
        Fields::Unnamed(f) if f.unnamed.len() == 1 => {
            let field = f.unnamed.first().unwrap();
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
            let ident = field.ident.as_ref().expect("named field has ident");
            let access = quote!(self.#ident);
            let ctor = quote!(Self { #ident: value });
            Ok((input.span(), access, ctor, Some(ident), &field.ty))
        }
        _ => Err(err()),
    }
}
