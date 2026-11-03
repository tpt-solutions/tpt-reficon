//! Lowering of predicate expressions into the verifier's constraint IR.
//!
//! The supported fragment is intentionally small and *statically decidable*:
//! boolean combinations (`&&`, `||`) of linear comparisons (`==`, `!=`, `<`,
//! `<=`, `>`, `>=`), parentheses, integer literals, `+`/`-`, unary `-`, and the
//! literals `true`/`false`. Anything else is reported with a span-accurate
//! error rather than being silently skipped, because a silent fallback would
//! weaken the compile-time guarantee.
//!
//! Formulas are normalized to **disjunctive normal form** (DNF): a list of
//! branches, each branch a conjunction of [`Constraint`]s. The formula holds
//! when *any* branch holds. This is what lets `||` be decided exactly: a
//! disjunction is unsatisfiable only when every branch is, and the type's
//! domain elides the check only when it entails some branch (see `expand.rs`).

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{BinOp, Expr, Lit, UnOp};

use tpt_reficon_verifier::ir::{Constraint, Linear, Relation};
use tpt_reficon_verifier::solver::{entails, unsat};

/// A formula in disjunctive normal form: a disjunction of conjunctions.
///
/// - `vec![]` represents `false` (no branch holds).
/// - `vec![vec![]]` represents `true` (one empty conjunction).
pub type Branches = Vec<Vec<Constraint>>;

/// Upper bound on how many branches a predicate may expand to. A pathological
/// chain of `||`/`&&` could otherwise exhaust the compiler's memory; beyond the
/// limit the predicate is rejected with a span-accurate error instead.
pub const MAX_DNF_BRANCHES: usize = 256;

/// Lowers a predicate expression into disjunctive normal form.
///
/// `allowed_vars` is the set of variable identifiers predicates may mention
/// (for `#[derive(Refine)]` that is just the refined value's variable).
pub fn lower(expr: &Expr, allowed_vars: &[String]) -> syn::Result<Branches> {
    match expr {
        Expr::Paren(p) => lower(&p.expr, allowed_vars),
        Expr::Group(p) => lower(&p.expr, allowed_vars),
        Expr::Lit(l) => match &l.lit {
            Lit::Bool(b) => {
                if b.value {
                    Ok(vec![Vec::new()]) // `true` — one empty conjunction
                } else {
                    Ok(Vec::new()) // `false` — no branch holds
                }
            }
            _ => Err(unsupported(expr, "expected a boolean predicate")),
        },
        Expr::Binary(b) if matches!(b.op, BinOp::And(_)) => conjoin(&[
            lower(&b.left, allowed_vars)?,
            lower(&b.right, allowed_vars)?,
        ])
        .ok_or_else(|| too_many_branches(b.op.span())),
        Expr::Binary(b) if matches!(b.op, BinOp::Or(_)) => {
            let mut out = lower(&b.left, allowed_vars)?;
            out.extend(lower(&b.right, allowed_vars)?);
            if out.len() > MAX_DNF_BRANCHES {
                return Err(too_many_branches(expr.span()));
            }
            Ok(out)
        }
        Expr::Binary(b) if is_comparison(&b.op) => {
            let rel = comparison_rel(&b.op, b.op.span())?;
            comparison_branch(expr, &b.left, &b.right, rel, allowed_vars)
        }
        Expr::Unary(u) if matches!(u.op, UnOp::Not(_)) => lower_negated(&u.expr, allowed_vars),
        other => Err(unsupported(other, "unsupported predicate shape")),
    }
}

/// The single-constraint branch `left (rel) right`, as `left - right (rel) 0`.
fn comparison_branch(
    expr: &Expr,
    left: &Expr,
    right: &Expr,
    rel: Relation,
    allowed_vars: &[String],
) -> syn::Result<Branches> {
    let lhs = linear(left, allowed_vars)?;
    let rhs = linear(right, allowed_vars)?;
    Ok(vec![vec![Constraint(
        lhs.checked_sub(&rhs)
            .ok_or_else(|| unsupported(expr, "coefficient overflow"))?,
        rel,
    )]])
}

/// Lowers `!expr` by pushing the negation inward. Comparisons flip, and
/// De Morgan's laws turn `!(a && b)` into `!a || !b` and `!(a || b)` into
/// `!a && !b`. The result is the DNF of the negation, not a negation of a DNF.
fn lower_negated(expr: &Expr, allowed_vars: &[String]) -> syn::Result<Branches> {
    match expr {
        Expr::Paren(p) => lower_negated(&p.expr, allowed_vars),
        Expr::Group(p) => lower_negated(&p.expr, allowed_vars),
        Expr::Unary(u) if matches!(u.op, UnOp::Not(_)) => lower(&u.expr, allowed_vars),
        Expr::Lit(l) => match &l.lit {
            Lit::Bool(b) => lower(
                &Expr::Lit(syn::ExprLit {
                    attrs: Vec::new(),
                    lit: Lit::Bool(syn::LitBool::new(!b.value, b.span)),
                }),
                allowed_vars,
            ),
            _ => Err(unsupported(expr, "expected a boolean predicate")),
        },
        Expr::Binary(b) if matches!(b.op, BinOp::And(_)) => {
            // !(a && b) = !a || !b
            let mut out = lower_negated(&b.left, allowed_vars)?;
            out.extend(lower_negated(&b.right, allowed_vars)?);
            if out.len() > MAX_DNF_BRANCHES {
                return Err(too_many_branches(expr.span()));
            }
            Ok(out)
        }
        Expr::Binary(b) if matches!(b.op, BinOp::Or(_)) => {
            // !(a || b) = !a && !b
            conjoin(&[
                lower_negated(&b.left, allowed_vars)?,
                lower_negated(&b.right, allowed_vars)?,
            ])
            .ok_or_else(|| too_many_branches(b.op.span()))
        }
        Expr::Binary(b) if is_comparison(&b.op) => {
            let rel = negate_relation(comparison_rel(&b.op, b.op.span())?);
            comparison_branch(expr, &b.left, &b.right, rel, allowed_vars)
        }
        other => Err(unsupported(other, "cannot negate this predicate shape")),
    }
}

/// The relation that holds exactly when `rel` does not.
fn negate_relation(rel: Relation) -> Relation {
    match rel {
        Relation::Eq => Relation::Ne,
        Relation::Ne => Relation::Eq,
        Relation::Lt => Relation::Ge,
        Relation::Ge => Relation::Lt,
        Relation::Le => Relation::Gt,
        Relation::Gt => Relation::Le,
    }
}

/// Conjoins formulas already in DNF, distributing `&&` over `||` (cartesian
/// product of branches). Returns `None` if the result exceeds
/// [`MAX_DNF_BRANCHES`].
pub fn conjoin(forms: &[Branches]) -> Option<Branches> {
    let mut acc: Branches = vec![Vec::new()];
    for form in forms {
        // `a && false` is `false`: no branch can hold.
        if form.is_empty() {
            return Some(Vec::new());
        }
        let mut next: Branches = Vec::new();
        for a in &acc {
            for b in form {
                let mut merged = a.clone();
                merged.extend(b.iter().cloned());
                next.push(merged);
                if next.len() > MAX_DNF_BRANCHES {
                    return None;
                }
            }
        }
        acc = next;
    }
    Some(acc)
}

/// True when no assignment satisfies the formula (every branch is UNSAT).
pub fn is_unsat(branches: &Branches) -> bool {
    branches.iter().all(|branch| unsat(branch))
}

/// True when `domain` alone already proves the formula.
///
/// Domain is a conjunction, so it entails a disjunction when it entails *some*
/// branch (that branch implies the whole formula). This is sound but
/// deliberately incomplete: a tautology whose branches are only jointly
/// exhaustive (e.g. `value == 0 || value != 0`) is not detected and instead
/// keeps an always-succeeding runtime check.
pub fn entailed_by(domain: &[Constraint], branches: &Branches) -> bool {
    branches
        .iter()
        .any(|branch| branch.iter().all(|c| entails(domain, c)))
}

fn too_many_branches(span: Span) -> syn::Error {
    syn::Error::new(
        span,
        format!(
            "refinement predicate expands to more than {MAX_DNF_BRANCHES} alternatives; \
             simplify the disjunction"
        ),
    )
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
                let lhs = linear(&b.left, allowed_vars)?;
                let rhs = linear(&b.right, allowed_vars)?;
                lhs.checked_add(&rhs)
                    .ok_or_else(|| unsupported(expr, "coefficient overflow"))
            }
            BinOp::Sub(_) => {
                let lhs = linear(&b.left, allowed_vars)?;
                let rhs = linear(&b.right, allowed_vars)?;
                lhs.checked_sub(&rhs)
                    .ok_or_else(|| unsupported(expr, "coefficient overflow"))
            }
            // Multiplication stays linear only when one side is a constant.
            BinOp::Mul(_) => {
                let lhs = linear(&b.left, allowed_vars)?;
                let rhs = linear(&b.right, allowed_vars)?;
                let (scalar, other) = if lhs.is_constant() {
                    (lhs.constant(), rhs)
                } else if rhs.is_constant() {
                    (rhs.constant(), lhs)
                } else {
                    return Err(unsupported(expr, "multiplication of two variables is not linear"));
                };
                other
                    .checked_scale(scalar)
                    .ok_or_else(|| unsupported(expr, "coefficient overflow"))
            }
            other => Err(syn::Error::new(
                other.span(),
                "unsupported binary operator in refinement predicate (expected +, -, or * by a constant)",
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
            "{msg}; the current frontend supports boolean combinations (`&&`, `||`) of \
             linear comparisons over the refined variable (`+`, `-`, integer literals), \
             e.g. `value > 0 && value <= 100` or `value == 0 || value == 1`"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    fn vars() -> Vec<String> {
        vec!["value".to_string()]
    }

    fn lower_ok(expr: Expr) -> Branches {
        lower(&expr, &vars()).expect("predicate should lower")
    }

    #[test]
    fn conjunction_is_one_branch() {
        let dnf = lower_ok(parse_quote!(value > 0 && value <= 100));
        assert_eq!(dnf.len(), 1);
        assert_eq!(dnf[0].len(), 2);
    }

    #[test]
    fn disjunction_is_one_branch_per_alternative() {
        let dnf = lower_ok(parse_quote!(value == 0 || value == 1));
        assert_eq!(dnf.len(), 2);
        assert!(dnf.iter().all(|b| b.len() == 1));
    }

    #[test]
    fn and_of_or_distributes() {
        // (a || b) && c → [a, c], [b, c]
        let dnf = lower_ok(parse_quote!((value == 0 || value == 1) && value <= 10));
        assert_eq!(dnf.len(), 2);
        assert!(dnf.iter().all(|b| b.len() == 2));
    }

    #[test]
    fn or_of_and_flattens() {
        // (a && b) || c → [a, b], [c]
        let dnf = lower_ok(parse_quote!((value > 0 && value < 5) || value == 9));
        assert_eq!(dnf.len(), 2);
        assert_eq!(dnf[0].len(), 2);
        assert_eq!(dnf[1].len(), 1);
    }

    #[test]
    fn boolean_literals() {
        // `true` is one empty branch; `false` has no branches.
        assert_eq!(lower_ok(parse_quote!(true)), vec![vec![]]);
        assert!(lower_ok(parse_quote!(false)).is_empty());
        // `true && x` keeps just `x`; `false || x` keeps just `x`.
        assert_eq!(lower_ok(parse_quote!(true && value == 3)).len(), 1);
        assert_eq!(lower_ok(parse_quote!(false || value == 3)).len(), 1);
    }

    #[test]
    fn unsatisfiability_is_branch_wise() {
        assert!(is_unsat(&lower_ok(parse_quote!(value > 5 && value < 3))));
        assert!(is_unsat(&lower_ok(parse_quote!(
            (value > 5 && value < 3) || (value > 9 && value < 4)
        ))));
        // One satisfiable alternative is enough for the formula to be SAT.
        assert!(!is_unsat(&lower_ok(parse_quote!(
            (value > 5 && value < 3) || value == 7
        ))));
        assert!(!is_unsat(&lower_ok(parse_quote!(value == 0 || value == 1))));
    }

    #[test]
    fn entailment_is_branch_wise() {
        // u8 domain: 0 <= value <= 255.
        let domain = vec![
            Constraint(Linear::var("value"), Relation::Ge),
            Constraint(
                Linear::var("value")
                    .checked_add(&Linear::constant_only(-255))
                    .unwrap(),
                Relation::Le,
            ),
        ];
        // Second alternative (`value <= 255`) is implied by the domain.
        assert!(entailed_by(
            &domain,
            &lower_ok(parse_quote!(value <= 3 || value <= 255))
        ));
        // Neither alternative is implied by the domain.
        assert!(!entailed_by(
            &domain,
            &lower_ok(parse_quote!(value == 0 || value == 1))
        ));
        // Must not panic on `false` (no branches → not entailed).
        assert!(!entailed_by(&domain, &lower_ok(parse_quote!(false))));
    }

    #[test]
    fn conjunction_blow_up_is_rejected() {
        // Conjoining two-way disjunctions multiplies branches; 10 of them
        // (2^10) exceed the cap and must be rejected rather than expanded.
        let mut expr: Expr = parse_quote!(value == 0 || value == 1);
        for k in 0..9 {
            let a = 2 * k + 2;
            let b = 2 * k + 3;
            let rhs: Expr = syn::parse_str(&format!("(value == {a} || value == {b})")).unwrap();
            expr = parse_quote!((#expr) && (#rhs));
        }
        let err = lower(&expr, &vars()).expect_err("must reject blow-up");
        assert!(
            err.to_string().contains("alternatives"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn conjoin_respects_branch_cap() {
        // Nine copies of a two-alternative form → 2^9 = 512 branches, which
        // exceeds the cap and must be rejected without expanding.
        let two: Branches = vec![Vec::new(), Vec::new()];
        let forms: Vec<Branches> = (0..9).map(|_| two.clone()).collect();
        assert!(conjoin(&forms).is_none());
        // A single form is returned essentially unchanged.
        assert_eq!(conjoin(&[two]).unwrap().len(), 2);
    }

    fn lower_str(src: &str) -> syn::Result<Branches> {
        let expr: Expr = syn::parse_str(src).unwrap();
        lower(&expr, &["x".to_string()])
    }

    #[test]
    fn negated_comparison_flips_the_relation() {
        let branches = lower_str("!(x < 5)").unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0][0].1, Relation::Ge);
        let eq = lower_str("!(x == 5)").unwrap();
        assert_eq!(eq[0][0].1, Relation::Ne);
    }

    #[test]
    fn negated_conjunction_is_a_disjunction() {
        // !(x > 0 && x < 9) == x <= 0 || x >= 9: two branches.
        assert_eq!(lower_str("!(x > 0 && x < 9)").unwrap().len(), 2);
    }

    #[test]
    fn negated_disjunction_is_a_conjunction() {
        // !(x < 0 || x > 9) == x >= 0 && x <= 9: one branch, two constraints.
        let branches = lower_str("!(x < 0 || x > 9)").unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].len(), 2);
    }

    #[test]
    fn double_negation_is_identity() {
        let branches = lower_str("!!(x == 3)").unwrap();
        assert_eq!(branches[0][0].1, Relation::Eq);
    }

    #[test]
    fn multiplication_by_a_constant_is_linear() {
        // 2 * x < 10 and x * 3 < 9 are both linear.
        assert!(lower_str("2 * x < 10").is_ok());
        assert!(lower_str("x * 3 < 9").is_ok());
    }

    #[test]
    fn multiplication_of_two_variables_is_rejected() {
        let err = lower_str("x * x < 10").unwrap_err().to_string();
        assert!(err.contains("not linear"), "{err}");
    }
}
