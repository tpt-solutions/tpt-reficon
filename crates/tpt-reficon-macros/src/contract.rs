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
use syn::visit_mut::{self, VisitMut};
use syn::{Block, Expr, FnArg, Ident, ItemFn, Pat, ReturnType, Type};

use tpt_reficon_verifier::ir::{Constraint, Linear};
use tpt_reficon_verifier::solver::unsat_integer;

use crate::{domain, integer, predicate};

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
    mutable: bool,
}

/// The name of the snapshot that holds the entry value of parameter `name`.
/// `old(x)` in an `#[ensures]` refers to this snapshot.
fn old_var(name: &str) -> String {
    format!("__tpt_old_{name}")
}

/// The `arg` of `old(arg)`, when `expr` is such a call with one plain identifier.
fn old_argument(expr: &Expr) -> Option<&Ident> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Path(func) = &*call.func else {
        return None;
    };
    if !func.path.is_ident("old") || call.args.len() != 1 {
        return None;
    }
    let Expr::Path(arg) = &call.args[0] else {
        return None;
    };
    arg.path.get_ident()
}

/// Replaces every `old(x)` for an integer parameter `x` with its snapshot. Also
/// returns the parameters that were named, in first-use order.
struct OldRewriter<'a> {
    params: &'a [String],
    found: Vec<String>,
    error: Option<syn::Error>,
}

impl VisitMut for OldRewriter<'_> {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if let Some(arg) = old_argument(expr).cloned() {
            let name = arg.to_string();
            if self.params.contains(&name) {
                if !self.found.contains(&name) {
                    self.found.push(name.clone());
                }
                let snapshot = Ident::new(&old_var(&name), arg.span());
                *expr = syn::parse_quote!(#snapshot);
            } else if self.error.is_none() {
                self.error = Some(syn::Error::new(
                    arg.span(),
                    format!("`old({name})` needs an integer parameter"),
                ));
            }
            return;
        }
        visit_mut::visit_expr_mut(self, expr);
    }
}

fn rewrite_old(exprs: &[Expr], params: &[String]) -> syn::Result<(Vec<Expr>, Vec<String>)> {
    let mut out = exprs.to_vec();
    let mut rewriter = OldRewriter {
        params,
        found: Vec::new(),
        error: None,
    };
    for expr in &mut out {
        rewriter.visit_expr_mut(expr);
    }
    if let Some(err) = rewriter.error {
        return Err(err);
    }
    Ok((out, rewriter.found))
}

/// Renames the parameters in an entry-state branch onto their snapshots. The
/// names are distinct before and after, so no coefficients merge.
fn to_entry(branch: &[Constraint], params: &[String]) -> Vec<Constraint> {
    branch
        .iter()
        .map(|Constraint(lin, rel)| {
            let terms = lin
                .terms()
                .iter()
                .map(|(v, c)| {
                    let name = if params.contains(v) {
                        old_var(v)
                    } else {
                        v.clone()
                    };
                    (name, *c)
                })
                .collect();
            let renamed = Linear::from_parts(terms, lin.constant())
                .expect("renaming distinct variables cannot merge coefficients");
            Constraint(renamed, *rel)
        })
        .collect()
}

/// Binds `id` to its `i128` widening inside a `'contract` block. A value that
/// does not fit in `i128` (only `u128` can) breaks out with `false`, so the
/// predicate is treated as violated rather than evaluated on a wrapped value.
fn widen_stmt(id: &Ident) -> TokenStream {
    quote! {
        #[allow(unused_variables)]
        let #id: i128 = match ::core::convert::TryInto::<i128>::try_into(#id) {
            ::core::result::Result::Ok(v) => v,
            ::core::result::Result::Err(_) => break 'contract false,
        };
    }
}

/// True when the constraints admit no value: unsatisfiable over ℚ, or, for one
/// integer variable, no integer in the domain either.
fn infeasible(cs: &[Constraint], int_var: Option<&str>) -> bool {
    unsat_integer(cs)
        || int_var.is_some_and(|v| {
            matches!(
                integer::analyze(cs, v),
                integer::IntegerFeasibility::Infeasible
            )
        })
}

/// Expands `#[contract]` on a function.
pub fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    expand_named(attr, item, None)
}

/// Expands `#[contract]`, naming the function `display_name` in panic messages
/// instead of its identifier. Generated helpers use this so that messages name
/// the function the user wrote.
pub(crate) fn expand_named(
    attr: TokenStream,
    item: TokenStream,
    display_name: Option<&str>,
) -> syn::Result<TokenStream> {
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
    // Panics come from the generated checks. `track_caller` reports them at the
    // call site, where the violation happened, not inside this function.
    if !f.attrs.iter().any(|a| a.path().is_ident("track_caller")) {
        f.attrs.push(syn::parse_quote!(#[track_caller]));
    }
    if pre_exprs.is_empty() && post_exprs.is_empty() {
        return Err(syn::Error::new(
            f.sig.ident.span(),
            "`#[contract]` needs at least one `#[requires(...)]` or `#[ensures(...)]`",
        ));
    }

    let params = collect_params(&f)?;
    let int_params: Vec<&Param> = params.iter().filter(|p| p.integer).collect();
    let int_names: Vec<String> = int_params.iter().map(|p| p.name.clone()).collect();
    // The integer-gap analysis handles one variable. With exactly one integer
    // parameter it runs on that parameter, otherwise it is skipped.
    let pre_var: Option<&str> = match int_names.as_slice() {
        [only] => Some(only.as_str()),
        _ => None,
    };
    // A `mut` parameter can be reassigned by the body, so a precondition holds
    // at exit only if the body leaves it alone. See the `post_elided` note.
    let has_mut_int = int_params.iter().any(|p| p.mutable);

    // `%` is sound only over unsigned integers. See `domain::is_unsigned`.
    let remainder_used = pre_exprs
        .iter()
        .chain(post_exprs.iter())
        .any(predicate::uses_remainder);
    if remainder_used && int_params.iter().any(|p| !domain::is_unsigned(&p.ty)) {
        return Err(syn::Error::new(
            f.sig.ident.span(),
            "`%` in a contract needs every integer parameter to be unsigned",
        ));
    }

    // `old(x)` reads the entry value of `x`. Each named parameter gets a
    // snapshot, taken before the body runs. Messages keep the source text.
    let post_src = post_exprs.clone();
    let (post_exprs, old_names) = rewrite_old(&post_exprs, &int_names)?;
    let old_params: Vec<(&Param, Ident)> = int_params
        .iter()
        .filter(|p| old_names.contains(&p.name))
        .map(|p| (*p, Ident::new(&old_var(&p.name), Span::call_site())))
        .collect();

    // Domain premises for every integer parameter.
    let mut premises: Vec<Constraint> = Vec::new();
    for p in &int_params {
        premises.extend(domain::domain_constraints(&p.ty, &p.name));
    }
    // A snapshot holds a value of its parameter's type, so it keeps the domain.
    let old_domain: Vec<Constraint> = int_params
        .iter()
        .flat_map(|p| domain::domain_constraints(&p.ty, &old_var(&p.name)))
        .collect();

    let fn_name = display_name.map_or_else(|| f.sig.ident.to_string(), str::to_string);
    let ret_ty: Type = match &f.sig.output {
        ReturnType::Default => syn::parse_quote!(()),
        ReturnType::Type(_, ty) => (**ty).clone(),
    };
    let ret_is_int = domain::is_supported_integer(&ret_ty);

    // Preconditions: lower, reject if no arguments can satisfy them (over ℚ,
    // or over ℤ for a single integer parameter), then decide whether the
    // parameter domains already prove them.
    let pre_dnf = lower_all(&pre_exprs, &int_names)?;
    let feasible = pre_dnf.iter().any(|branch| {
        let mut combined = premises.clone();
        combined.extend(branch.iter().cloned());
        !infeasible(&combined, pre_var)
    });
    if !feasible {
        return Err(syn::Error::new(
            f.sig.ident.span(),
            format!(
                "unsatisfiable contract on `{fn_name}`: no arguments satisfy `{}`{}",
                render(&pre_exprs),
                predicate::conflict_note(&pre_dnf)
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
    post_allowed.extend(old_names.iter().map(|n| old_var(n)));
    let post_dnf = lower_all(&post_exprs, &post_allowed)?;
    let mut post_premises = premises.clone();
    post_premises.extend(old_domain.iter().cloned());
    if ret_is_int {
        post_premises.extend(domain::domain_constraints(&ret_ty, RESULT));
    }
    let post_var = if ret_is_int { Some(RESULT) } else { None };

    // A postcondition that no entry state and no result can satisfy fails on
    // every call that returns. Reject it now, the way an unsatisfiable
    // precondition is rejected.
    if !post_exprs.is_empty() {
        // As in `post_elided` below, a `mut` parameter's entry precondition does
        // not constrain the exit value that `ensures` sees.
        let impossible = pre_dnf.iter().all(|pre_branch| {
            post_dnf.iter().all(|post_branch| {
                let mut combined = post_premises.clone();
                if !has_mut_int {
                    combined.extend(pre_branch.iter().cloned());
                }
                combined.extend(to_entry(pre_branch, &int_names));
                combined.extend(post_branch.iter().cloned());
                infeasible(&combined, post_var)
            })
        });
        if impossible {
            return Err(syn::Error::new(
                f.sig.ident.span(),
                format!(
                    "unsatisfiable postcondition on `{fn_name}`: no result satisfies `{}`",
                    render(&post_src)
                ),
            ));
        }
    }

    // The postcondition may only use the entry preconditions on the exit values
    // when the body cannot change the parameters. A `mut` parameter is
    // reassigned at will, so `ensures` on the exit value skips that elision.
    // The renamed entry premises are always sound: they talk about snapshots.
    let post_elided = pre_dnf.iter().all(|branch| {
        let mut combined = post_premises.clone();
        if !has_mut_int {
            combined.extend(branch.iter().cloned());
        }
        combined.extend(to_entry(branch, &int_names));
        predicate::entailed_by(&combined, &post_dnf)
    }) && !has_mut_int;

    let pre_check = if pre_elided || pre_exprs.is_empty() {
        TokenStream::new()
    } else {
        let msg = format!(
            "precondition `{}` of `{fn_name}` violated",
            render(&pre_exprs)
        );
        let preds = &pre_exprs;
        let widen: Vec<TokenStream> = int_params.iter().map(|p| widen_stmt(&p.ident)).collect();
        quote! {
            if !('contract: {
                #(#widen)*
                (#( (#preds) )&&*)
            }) {
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
            render(&post_src)
        );
        let posts = &post_exprs;
        let snapshots: Vec<TokenStream> = old_params
            .iter()
            .map(|(p, snapshot)| {
                let (id, ty) = (&p.ident, &p.ty);
                quote!(let #snapshot: #ty = #id;)
            })
            .collect();
        let widen: Vec<TokenStream> = int_params
            .iter()
            .map(|p| widen_stmt(&p.ident))
            .chain(old_params.iter().map(|(_, snapshot)| widen_stmt(snapshot)))
            .collect();
        // `result` is the body's value, which lives in `temp` until returned.
        let widen_result = quote! {
            #[allow(unused_variables)]
            let #result_id: i128 = match ::core::convert::TryInto::<i128>::try_into(#temp) {
                ::core::result::Result::Ok(v) => v,
                ::core::result::Result::Err(_) => break 'contract false,
            };
        };
        quote! {
            #pre_check
            #(#snapshots)*
            let #temp: #ret = (|| -> #ret #block)();
            if !('contract: {
                #(#widen)*
                #widen_result
                (#( (#posts) )&&*)
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
    if matches!(sig.safety, syn::Safety::Unsafe(_)) {
        return unsupported(sig.span(), "`unsafe` functions");
    }
    if sig.abi.is_some() {
        return unsupported(sig.span(), "`extern` functions");
    }
    if sig.variadic.is_some() {
        return unsupported(sig.span(), "variadic functions");
    }
    // Lifetime parameters change nothing about the checks, so they are fine.
    if sig
        .generics
        .params
        .iter()
        .any(|p| !matches!(p, syn::GenericParam::Lifetime(_)))
    {
        return unsupported(sig.generics.span(), "generic functions");
    }
    Ok(())
}

fn collect_params(f: &ItemFn) -> syn::Result<Vec<Param>> {
    f.sig
        .inputs
        .iter()
        // The receiver is not an integer, so no check reads it. Its methods still
        // see it through the closure that wraps the body.
        .filter_map(|arg| match arg {
            FnArg::Receiver(_) => None,
            FnArg::Typed(pt) => Some(match pt.pat.as_ref() {
                Pat::Ident(pi) => {
                    let ty = (*pt.ty).clone();
                    Ok(Param {
                        name: pi.ident.to_string(),
                        ident: pi.ident.clone(),
                        integer: domain::is_supported_integer(&ty),
                        mutable: pi.mutability.is_some(),
                        ty,
                    })
                }
                other => Err(syn::Error::new(
                    other.span(),
                    "`#[contract]` parameters must be plain identifiers",
                )),
            }),
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
    fn methods_with_a_receiver_are_accepted() {
        let out = expand_str("#[requires(x > 0)] fn f(&self, x: i64) -> i64 { x }");
        assert!(out.contains("precondition"), "{out}");
    }

    #[test]
    fn result_parameter_conflicts_with_ensures() {
        let msg = expand_err("#[ensures(result > 0)] fn f(result: i64) -> i64 { result }");
        assert!(msg.contains("named `result`"), "{msg}");
    }

    #[test]
    fn mut_parameter_postcondition_is_checked_not_elided() {
        // The body reassigns `x` to -5, so the entry precondition says nothing
        // about the exit value that `ensures` checks.
        let out = expand_str(
            "#[requires(x > 0)] #[ensures(x > 0)] fn f(mut x: i64) -> i64 { x = -5; x }",
        );
        assert!(out.contains("postcondition"), "{out}");
    }

    #[test]
    fn mut_parameter_postcondition_is_not_rejected_as_impossible() {
        // Entry says `x > 0`, but the body sets `x = -1`, so `x < 0` at exit is
        // satisfiable. Entry premises must not make it look impossible.
        let out = expand_str(
            "#[requires(x > 0)] #[ensures(x < 0)] fn f(mut x: i64) -> i64 { x = -1; x }",
        );
        assert!(out.contains("postcondition"), "{out}");
    }

    #[test]
    fn immutable_parameter_postcondition_is_still_elided() {
        // Without `mut`, the precondition does hold at exit, so the check is
        // provably redundant and stays elided.
        let out = expand_str("#[requires(x > 0)] #[ensures(x > 0)] fn f(x: i64) -> i64 { x }");
        assert!(!out.contains("postcondition"), "{out}");
    }

    #[test]
    fn integer_gap_precondition_is_rejected() {
        // `x > 5 && x < 6` has rational solutions but no integer one.
        let msg = expand_err("#[requires(x > 5 && x < 6)] fn f(x: i64) {}");
        assert!(msg.contains("unsatisfiable contract"), "{msg}");
    }

    #[test]
    fn integer_gap_postcondition_is_rejected() {
        let msg = expand_err("#[ensures(result > 5 && result < 6)] fn f() -> i64 { 0 }");
        assert!(msg.contains("unsatisfiable postcondition"), "{msg}");
    }

    #[test]
    fn postcondition_outside_return_domain_is_rejected() {
        // `u8` results are at most 255, so `result > 300` can never hold.
        let msg = expand_err("#[ensures(result > 300)] fn f(x: i32) -> u8 { 0 }");
        assert!(msg.contains("unsatisfiable postcondition"), "{msg}");
    }

    #[test]
    fn u128_contract_values_are_not_wrapped_by_a_cast() {
        // `as i128` maps u128 values above i128::MAX to negative numbers.
        let out = expand_str("#[requires(x < 1000)] fn f(x: u128) -> u128 { x }");
        assert!(!out.contains("as i128"), "{out}");
        assert!(out.contains("TryInto"), "{out}");
    }
}

#[cfg(test)]
mod attribute_tests {
    use super::*;

    fn expand_str(src: &str) -> String {
        let item: TokenStream = src.parse().unwrap();
        expand(TokenStream::new(), item)
            .unwrap_or_else(|e| panic!("expansion failed: {e}"))
            .to_string()
    }

    #[test]
    fn lifetime_only_generics_are_accepted() {
        let out = expand_str("#[requires(x > 0)] fn f<'a>(x: i64) -> i64 { x }");
        assert!(out.contains("precondition"), "{out}");
    }

    #[test]
    fn suffixed_literals_lower() {
        let out = expand_str("#[requires(x < 5u8)] fn f(x: u8) -> u8 { x }");
        assert!(out.contains("precondition"), "{out}");
    }

    #[test]
    fn generated_checks_report_the_caller_location() {
        let out = expand_str("#[requires(x > 0)] fn f(x: i64) -> i64 { x }");
        assert!(out.contains("track_caller"), "{out}");
    }
}

#[cfg(test)]
mod old_tests {
    use super::*;

    fn expand_str(src: &str) -> String {
        let item: TokenStream = src.parse().unwrap();
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
    fn old_is_rewritten_to_a_snapshot_taken_before_the_body() {
        let out = expand_str("#[ensures(result == old(x) + 1)] fn f(x: u8) -> u8 { x + 1 }");
        assert!(out.contains("__tpt_old_x"), "{out}");
        assert!(out.contains("let __tpt_old_x : u8 = x"), "{out}");
        assert!(out.contains("postcondition"), "{out}");
    }

    #[test]
    fn old_of_a_non_parameter_is_rejected() {
        let msg = expand_err("#[ensures(result == old(y))] fn f(x: i64) -> i64 { x }");
        assert!(msg.contains("needs an integer parameter"), "{msg}");
    }

    #[test]
    fn old_in_a_precondition_is_rejected() {
        // `old` names an entry value, which a precondition cannot refer to.
        let item: TokenStream = "#[requires(old(x) > 0)] fn f(x: i64) {}".parse().unwrap();
        assert!(expand(TokenStream::new(), item).is_err());
    }
}

#[cfg(test)]
mod remainder_contract_tests {
    use super::*;

    #[test]
    fn remainder_in_a_contract_needs_unsigned_parameters() {
        let item: TokenStream = "#[requires(x % 2 == 0)] fn f(x: i64) {}".parse().unwrap();
        let msg = expand(TokenStream::new(), item).unwrap_err().to_string();
        assert!(
            msg.contains("every integer parameter to be unsigned"),
            "{msg}"
        );
        let item: TokenStream = "#[requires(x % 2 == 0)] fn f(x: u64) {}".parse().unwrap();
        assert!(expand(TokenStream::new(), item).is_ok());
    }
}

#[cfg(test)]
mod integer_gap_tests {
    use super::*;

    #[test]
    fn two_parameter_integer_gap_is_rejected() {
        // a + a + b + b == 3 has rational solutions and no integer ones, because
        // the left side is always even. Integer tightening finds that.
        let item: TokenStream = "#[requires(a + a + b + b == 3)] fn f(a: i64, b: i64) {}"
            .parse()
            .unwrap();
        let msg = expand(TokenStream::new(), item).unwrap_err().to_string();
        assert!(msg.contains("unsatisfiable contract"), "{msg}");
    }
}

#[cfg(test)]
mod conflict_contract_tests {
    use super::*;

    #[test]
    fn unsatisfiable_precondition_names_the_conflict() {
        let item: TokenStream = "#[requires(x > 5 && x < 3)] fn f(x: i64) {}"
            .parse()
            .unwrap();
        let msg = expand(TokenStream::new(), item).unwrap_err().to_string();
        assert!(msg.contains("conflicting constraints"), "{msg}");
    }
}
