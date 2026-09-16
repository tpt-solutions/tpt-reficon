//! Lowering of predicate expressions into the verifier's constraint IR.
//!
//! The supported fragment is intentionally small and *statically decidable*:
//! conjunctions of linear comparisons (`&&`, `==`, `!=`, `<`, `<=`, `>`,
//! `>=`, parentheses, integer literals, `+`/`-`, unary `-`). Anything else is
//! reported with a span-accurate error rather than being silently skipped,
//! because a silent fallback would weaken the compile-time guarantee.

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{BinOp, Expr, Lit, UnOp};

use tpt_reficon_verifier::ir::{Constraint, Linear, Relation};

/// Lowers a predicate expression into a conjunction of [`Constraint`]s.
///
/// `allowed_vars` is the set of variable identifiers predicates may mention
/// (for `#[derive(Refine)]` that is just the refined value's variable).
pub fn lower(expr: &Expr, allowed_vars: &[String]) -> syn::Result<Vec<Constraint>> {
    match expr {
        Expr::Paren(p) => lower(&p.expr, allowed_vars),
        Expr::Group(p) => lower(&p.expr, allowed_vars),
        Expr::Lit(l) => match &l.lit {
            Lit::Bool(b) => {
                if b.value {
                    Ok(Vec::new()) // `true` — no constraint
                } else {
                    // `false` — unsat marker: `0 >= 1`
                    Ok(vec![Constraint(Linear::constant_only(-1), Relation::Ge)])
                }
            }
            _ => Err(unsupported(expr, "expected a boolean predicate")),
        },
        Expr::Binary(b) if matches!(b.op, BinOp::And(_)) => {
            let mut out = lower(&b.left, allowed_vars)?;
            out.extend(lower(&b.right, allowed_vars)?);
            Ok(out)
        }
        Expr::Binary(b) if is_comparison(&b.op) => {
            let lhs = linear(&b.left, allowed_vars)?;
            let rhs = linear(&b.right, allowed_vars)?;
            let rel = comparison_rel(&b.op, b.op.span())?;
            Ok(vec![Constraint(lhs.sub(&rhs), rel)])
        }
        other => Err(unsupported(other, "unsupported predicate shape")),
    }
}

fn is_comparison(op: &BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq(_) | BinOp::Ne(_) | BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_)
    )
}

fn comparison_rel(op: &BinOp, span: Span) -> syn::Result<Relation> {
    match op {
        BinOp::Eq(_) => Ok(Relation::Eq),
        BinOp::Ne(_) => Ok(Relation::Ne),
        BinOp::Lt(_) => Ok(Relation::Lt),
        BinOp::Le(_) => Ok(Relation::Le),
        BinOp::Gt(_) => Ok(Relation::Gt),
        BinOp::Ge(_) => Ok(Relation::Ge),
        other => Err(syn::Error::new(
            span,
            format!("unsupported operator `{other:?}` in refinement predicate"),
        )),
    }
}

/// Builds an integer-linear expression. Identifiers must be in
/// `allowed_vars`; comparisons inside expressions are rejected by construction.
fn linear(expr: &Expr, allowed_vars: &[String]) -> syn::Result<Linear> {
    match expr {
        Expr::Paren(p) => linear(&p.expr, allowed_vars),
        Expr::Group(p) => linear(&p.expr, allowed_vars),
        Expr::Lit(l) => match &l.lit {
            Lit::Int(i) => {
                let v: i64 = i.base10_parse().map_err(|_| {
                    syn::Error::new(
                        i.span(),
                        "integer literal does not fit in i64 (the solver's coefficient range)",
                    )
                })?;
                Ok(Linear::constant_only(v))
            }
            _ => Err(unsupported(expr, "expected an integer literal")),
        },
        Expr::Unary(u) => match u.op {
            UnOp::Neg(_) => linear(&u.expr, allowed_vars)?
                .checked_scale(-1)
                .ok_or_else(|| {
                    syn::Error::new(
                        u.span(),
                        "negation overflowed the solver's coefficient range",
                    )
                }),
            other => Err(syn::Error::new(
                other.span(),
                "unsupported unary operator in refinement predicate",
            )),
        },
        Expr::Path(p) => {
            let ident = p.path.get_ident().ok_or_else(|| {
                unsupported(expr, "only plain identifiers are supported as variables")
            })?;
            if allowed_vars.iter().any(|v| v == &ident.to_string()) {
                Ok(Linear::var(ident.to_string()))
            } else {
                Err(syn::Error::new(
                    ident.span(),
                    format!(
                        "unknown variable `{ident}` in refinement predicate (refined value is `{}`)",
                        allowed_vars.join("`, `")
                    ),
                ))
            }
        }
        Expr::Binary(b) => match b.op {
            BinOp::Add(_) => {
                Ok(linear(&b.left, allowed_vars)?.add(&linear(&b.right, allowed_vars)?))
            }
            BinOp::Sub(_) => {
                Ok(linear(&b.left, allowed_vars)?.sub(&linear(&b.right, allowed_vars)?))
            }
            other => Err(syn::Error::new(
                other.span(),
                "unsupported binary operator in refinement predicate (expected + or -)",
            )),
        },
        other => Err(unsupported(
            other,
            "unsupported term in refinement predicate",
        )),
    }
}

fn unsupported(expr: &Expr, msg: &str) -> syn::Error {
    syn::Error::new(
        expr.span(),
        format!(
            "{msg}; the current frontend supports conjunctions of linear comparisons \
             over the refined variable (`+`, `-`, integer literals), e.g. \
             `value > 0 && value <= 100`"
        ),
    )
}
