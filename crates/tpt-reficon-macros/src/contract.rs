//! `#[contract]`: pre- and postconditions on plain functions.
//!
//! The design, and the reasons behind each choice, are in
//! `docs/design/function-contracts.md`. In short: predicates are lowered with
//! the same frontend as `#[derive(Refine)]`, a precondition is elided when the
//! parameter domains prove it, a postcondition is elided when the return-type
//! domain and the preconditions prove it, and everything else is a runtime
//! check that panics on violation.

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::{Block, Expr, FnArg, Ident, ItemFn, Pat, ReturnType, Type};

use tpt_reficon_verifier::ir::Constraint;
use tpt_reficon_verifier::solver::unsat;

use crate::{domain, predicate};

/// Name of the binding that holds the body's value while postconditions run.
/// Double underscores keep it clear of user identifiers.
const RESULT_TEMP: &str = "__tpt_result";

/// The name a postcondition uses for the return value.
const RESULT: &str = "result";

struct Param {
    name: String,
    ident: Ident,
    ty: Type,
    integer: bool,
}

/// Expands `#[contract]` on a function.
pub fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "`#[contract]` takes no arguments; put predicates in `#[requires(...)]` and `#[ensures(...)]`",
        ));
    }
    let mut f: ItemFn = syn::parse2(item)?;
    reject_unsupported(&f)?;

    let mut pre_exprs = Vec::new();
    let mut post_exprs = Vec::new();
    let mut kept = Vec::new();
    for attr in std::mem::take(&mut f.attrs) {
        if attr.path().is_ident("requires") {
            pre_exprs.push(attr.parse_args::<Expr>()?);
        } else if attr.path().is_ident("ensures") {
            post_exprs.push(attr.parse_args::<Expr>()?);
        } else {
            kept.push(attr);
        }
    }
    f.attrs = kept;
    if pre_exprs.is_empty() && post_exprs.is_empty() {
        return Err(syn::Error::new(
            f.sig.ident.span(),
            "`#[contract]` needs at least one `#[requires(...)]` or `#[ensures(...)]`",
        ));
    }

    let params = collect_params(&f)?;
    let int_params: Vec<&Param> = params.iter().filter(|p| p.integer).collect();
    let int_names: Vec<String> = int_params.iter().map(|p| p.name.clone()).collect();

    // Domain premises for every integer parameter.
    let mut premises: Vec<Constraint> = Vec::new();
    for p in &int_params {
        premises.extend(domain::domain_constraints(&p.ty, &p.name));
    }

    let fn_name = f.sig.ident.to_string();
    let ret_ty: Type = match &f.sig.output {
        ReturnType::Default => syn::parse_quote!(()),
        ReturnType::Type(_, ty) => (**ty).clone(),
    };
    let ret_is_int = domain::is_supported_integer(&ret_ty);

    // Preconditions: lower, reject if no arguments can satisfy them, then
    // decide whether the parameter domains already prove them.
    let pre_dnf = lower_all(&pre_exprs, &int_names)?;
    let feasible = pre_dnf.iter().any(|branch| {
        let mut combined = premises.clone();
        combined.extend(branch.iter().cloned());
        !unsat(&combined)
    });
    if !feasible {
        return Err(syn::Error::new(
            f.sig.ident.span(),
            format!(
                "unsatisfiable contract on `{fn_name}`: no arguments satisfy `{}`",
                render(&pre_exprs)
            ),
        ));
    }
    let pre_elided = predicate::entailed_by(&premises, &pre_dnf);

    // Postconditions: may mention `result`, so the return type must be an
    // integer. Elided when the premises at exit prove them. Those premises are
    // the parameter domains, the return-type domain, and each precondition
    // branch, which holds at exit because entry checked it or elided it.
    if !post_exprs.is_empty() && !ret_is_int {
        return Err(syn::Error::new(
            ret_ty.span(),
            "`#[ensures]` needs an integer return type, because it constrains `result`",
        ));
    }
    if !post_exprs.is_empty() && params.iter().any(|p| p.name == RESULT) {
        return Err(syn::Error::new(
            f.sig.inputs.span(),
            "a parameter named `result` conflicts with `#[ensures]`; rename it",
        ));
    }
    let mut post_allowed = int_names.clone();
    if ret_is_int {
        post_allowed.push(RESULT.to_string());
    }
    let post_dnf = lower_all(&post_exprs, &post_allowed)?;
    let mut post_premises = premises.clone();
    if ret_is_int {
        post_premises.extend(domain::domain_constraints(&ret_ty, RESULT));
    }
    let post_elided = pre_dnf.iter().all(|branch| {
        let mut combined = post_premises.clone();
        combined.extend(branch.iter().cloned());
        predicate::entailed_by(&combined, &post_dnf)
    });

    let widen: Vec<TokenStream> = int_params
        .iter()
        .map(|p| {
            let id = &p.ident;
            quote!(#[allow(unused_variables)] let #id: i128 = #id as i128;)
        })
        .collect();

    let pre_check = if pre_elided || pre_exprs.is_empty() {
        TokenStream::new()
    } else {
        let msg = format!(
            "precondition `{}` of `{fn_name}` violated",
            render(&pre_exprs)
        );
        let preds = &pre_exprs;
        quote! {
            if !({ #(#widen)* ((#( (#preds) )&&*)) }) {
                ::core::panic!(#msg)
            }
        }
    };

    let block = f.block.clone();
    let body = if post_exprs.is_empty() || post_elided {
        quote!(#pre_check #block)
    } else {
        let temp = Ident::new(RESULT_TEMP, Span::call_site());
        let ret = &ret_ty;
        let result_id = Ident::new(RESULT, Span::call_site());
        let msg = format!(
            "postcondition `{}` of `{fn_name}` violated",
            render(&post_exprs)
        );
        let posts = &post_exprs;
        quote! {
            #pre_check
            let #temp: #ret = (|| -> #ret #block)();
            if !({
                #(#widen)*
                #[allow(unused_variables)]
                let #result_id: i128 = #temp as i128;
                ((#( (#posts) )&&*))
            }) {
                ::core::panic!(#msg)
            }
            #temp
        }
    };

    f.block = Box::new(syn::parse2::<Block>(quote!({ #body }))?);
    Ok(f.into_token_stream())
}

fn reject_unsupported(f: &ItemFn) -> syn::Result<()> {
    let sig = &f.sig;
    let unsupported = |span: Span, what: &str| {
        Err(syn::Error::new(
            span,
            format!("`#[contract]` does not support {what}"),
        ))
    };
    if sig.asyncness.is_some() {
        return unsupported(sig.span(), "`async` functions");
    }
    if sig.constness.is_some() {
        return unsupported(sig.span(), "`const` functions");
    }
    if sig.unsafety.is_some() {
        return unsupported(sig.span(), "`unsafe` functions");
    }
    if sig.abi.is_some() {
        return unsupported(sig.span(), "`extern` functions");
    }
    if sig.variadic.is_some() {
        return unsupported(sig.span(), "variadic functions");
    }
    if !sig.generics.params.is_empty() {
        return unsupported(sig.generics.span(), "generic functions");
    }
    Ok(())
}

fn collect_params(f: &ItemFn) -> syn::Result<Vec<Param>> {
    f.sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Receiver(r) => Err(syn::Error::new(
                r.span(),
                "`#[contract]` does not support methods or `self`",
            )),
            FnArg::Typed(pt) => match pt.pat.as_ref() {
                Pat::Ident(pi) => {
                    let ty = (*pt.ty).clone();
                    Ok(Param {
                        name: pi.ident.to_string(),
                        ident: pi.ident.clone(),
                        integer: domain::is_supported_integer(&ty),
                        ty,
                    })
                }
                other => Err(syn::Error::new(
                    other.span(),
                    "`#[contract]` parameters must be plain identifiers",
                )),
            },
        })
        .collect()
}

fn lower_all(exprs: &[Expr], allowed: &[String]) -> syn::Result<predicate::Branches> {
    let forms = exprs
        .iter()
        .map(|e| predicate::lower(e, allowed))
        .collect::<syn::Result<Vec<_>>>()?;
    predicate::conjoin(&forms).ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "contract has too many predicate alternatives; simplify the disjunctions",
        )
    })
}

fn render(exprs: &[Expr]) -> String {
    if exprs.is_empty() {
        return "true".to_string();
    }
    exprs
        .iter()
        .map(|e| quote!(#e).to_string())
        .collect::<Vec<_>>()
        .join(" && ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand_str(src: &str) -> String {
        let item: TokenStream = src.parse().unwrap();
        // The attribute is consumed by `expand` as the item's own attributes.
        expand(TokenStream::new(), item)
            .unwrap_or_else(|e| panic!("expansion failed: {e}"))
            .to_string()
    }

    fn expand_err(src: &str) -> String {
        let item: TokenStream = src.parse().unwrap();
        match expand(TokenStream::new(), item) {
            Ok(_) => panic!("expected an error"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn precondition_proven_by_domain_is_elided() {
        let out = expand_str("#[requires(x <= 255)] fn f(x: u8) -> u8 { x }");
        assert!(!out.contains("panic"), "{out}");
    }

    #[test]
    fn precondition_not_proven_by_domain_is_checked() {
        let out = expand_str("#[requires(x <= 100)] fn f(x: u8) -> u8 { x }");
        assert!(out.contains("panic"), "{out}");
    }

    #[test]
    fn postcondition_proven_by_return_domain_is_elided() {
        let out = expand_str("#[ensures(result <= 255)] fn f(x: i32) -> u8 { x as u8 }");
        assert!(!out.contains("panic"), "{out}");
    }

    #[test]
    fn postcondition_depending_on_body_is_checked() {
        let out = expand_str("#[ensures(result >= 0)] fn f(x: i32) -> i32 { x }");
        assert!(out.contains("panic"), "{out}");
    }

    #[test]
    fn postcondition_entailed_by_precondition_is_elided() {
        // `a == 0` at entry still holds at exit, so the postcondition is
        // proven without looking at the body.
        let out = expand_str("#[requires(a == 0)] #[ensures(a == 0)] fn f(a: u8) -> u8 { a }");
        assert!(!out.contains("postcondition"), "{out}");
    }

    #[test]
    fn postcondition_on_result_is_not_proven_from_preconditions() {
        // `result == a + 1` depends on the body, which the solver never sees,
        // so the check must stay.
        let out = expand_str(
            "#[requires(a == 0)] #[ensures(result == a + 1)] fn f(a: u8) -> u8 { a + 1 }",
        );
        assert!(out.contains("postcondition"), "{out}");
    }

    #[test]
    fn unsatisfiable_precondition_is_rejected() {
        let msg = expand_err("#[requires(x > 5 && x < 3)] fn f(x: i64) {}");
        assert!(msg.contains("unsatisfiable contract"), "{msg}");
    }

    #[test]
    fn missing_predicates_are_rejected() {
        let msg = expand_err("fn f(x: i64) {}");
        assert!(msg.contains("needs at least one"), "{msg}");
    }

    #[test]
    fn methods_are_rejected() {
        let msg = expand_err("#[requires(x > 0)] fn f(&self, x: i64) {}");
        assert!(msg.contains("methods"), "{msg}");
    }

    #[test]
    fn result_parameter_conflicts_with_ensures() {
        let msg = expand_err("#[ensures(result > 0)] fn f(result: i64) -> i64 { result }");
        assert!(msg.contains("named `result`"), "{msg}");
    }
}
