//! The internal constraint representation for QF_LRA.
//!
//! This is the crate-level "constraint IR" that replaced the SMT-LIB2
//! generator concept from `spec.txt` (see `CHANGELOG.md` — deviation note):
//! predicates are lowered straight into [`Constraint`]s and handed to the
//! Fourier–Motzkin engine, no external solver syntax involved.
//!
//! The shape intentionally mirrors the ported `tpt-telos` verifier:
//! `Linear { terms, constant }`, `Relation::{Le,Lt,Ge,Gt,Eq,Ne}`,
//! `Constraint(Linear, Relation)`.

use core::cmp::Ordering;
use core::fmt;
use std::collections::BTreeMap;

use crate::rational::Rat;

/// A relation between a linear expression and zero.
///
/// The constraint `Constraint(lin, rel)` asserts `lin (rel) 0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    /// `<=`
    Le,
    /// `<`
    Lt,
    /// `>=`
    Ge,
    /// `>`
    Gt,
    /// `==`
    Eq,
    /// `!=`
    Ne,
}

impl fmt::Display for Relation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Relation::Le => "<=",
            Relation::Lt => "<",
            Relation::Ge => ">=",
            Relation::Gt => ">",
            Relation::Eq => "==",
            Relation::Ne => "!=",
        })
    }
}

/// A linear expression: `sum((var_i, coeff_i)) + constant`.
///
/// Coefficients are exact `i64`; the solver widens to rational arithmetic
/// internally. The fields are private so that every expression goes through a
/// constructor that merges repeated variables, and every combination is
/// checked: an overflowing coefficient is reported as `None`, never wrapped
/// or saturated into a different constraint.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Linear {
    /// `(variable name, coefficient)` pairs, one per variable; zero
    /// coefficients removed.
    terms: Vec<(String, i64)>,
    /// Additive constant.
    constant: i64,
}

impl Linear {
    /// The expression consisting of a single variable with coefficient 1.
    pub fn var(name: impl Into<String>) -> Linear {
        Linear {
            terms: vec![(name.into(), 1)],
            constant: 0,
        }
    }

    /// A constant-only expression.
    pub fn constant_only(v: i64) -> Linear {
        Linear {
            terms: Vec::new(),
            constant: v,
        }
    }

    /// Builds an expression from raw parts, merging repeated variables.
    ///
    /// Returns `None` when a merged coefficient overflows `i64`.
    pub fn from_parts(terms: Vec<(String, i64)>, constant: i64) -> Option<Linear> {
        let mut out = Linear::constant_only(constant);
        for (v, c) in terms {
            out.add_term(&v, c)?;
        }
        Some(out)
    }

    /// The `(variable, coefficient)` pairs, one entry per variable.
    pub fn terms(&self) -> &[(String, i64)] {
        &self.terms
    }

    /// The additive constant.
    pub fn constant(&self) -> i64 {
        self.constant
    }

    /// True if the expression mentions no variables.
    pub fn is_constant(&self) -> bool {
        self.terms.is_empty()
    }

    /// Adds `c · v` to the expression, merging with an existing entry for `v`.
    fn add_term(&mut self, v: &str, c: i64) -> Option<()> {
        if let Some(slot) = self.terms.iter_mut().find(|(vv, _)| vv == v) {
            slot.1 = slot.1.checked_add(c)?;
        } else {
            self.terms.push((v.to_string(), c));
        }
        self.terms.retain(|(_, c)| *c != 0);
        Some(())
    }

    /// Adds two expressions, merging like terms. `None` on overflow.
    pub fn checked_add(&self, rhs: &Linear) -> Option<Linear> {
        self.combine(rhs, false)
    }

    /// Subtracts `rhs` from `self`. `None` on overflow.
    pub fn checked_sub(&self, rhs: &Linear) -> Option<Linear> {
        self.combine(rhs, true)
    }

    fn combine(&self, rhs: &Linear, negate: bool) -> Option<Linear> {
        let signed = |c: i64| if negate { c.checked_neg() } else { Some(c) };
        let mut out = self.clone();
        out.constant = out.constant.checked_add(signed(rhs.constant)?)?;
        for (v, c) in &rhs.terms {
            out.add_term(v, signed(*c)?)?;
        }
        Some(out)
    }

    /// Multiplies all terms and the constant by an integer scalar.
    ///
    /// Returns `None` on overflow (callers then stay conservative).
    pub fn checked_scale(&self, k: i64) -> Option<Linear> {
        let mut terms = Vec::with_capacity(self.terms.len());
        for (v, c) in &self.terms {
            terms.push((v.clone(), c.checked_mul(k)?));
        }
        Some(Linear {
            terms,
            constant: self.constant.checked_mul(k)?,
        })
    }

    /// Evaluates the expression under a model.
    ///
    /// Returns `None` if any intermediate multiply or add overflows the
    /// rational's `i128` storage. Callers must treat this as "cannot verify"
    /// (never as a violation), mirroring the checked-arithmetic policy of the
    /// rest of the solver.
    pub fn checked_eval(&self, model: &Model) -> Option<Rat> {
        let mut acc = Rat::from_i128(self.constant as i128);
        for (v, c) in &self.terms {
            let val = model.get(v).copied().unwrap_or(Rat::ZERO);
            acc = acc.checked_add(val.checked_scale(*c as i128)?)?;
        }
        Some(acc)
    }

    /// Evaluates the expression under a model. Panics on arithmetic overflow,
    /// which cannot happen for models produced by this crate on systems that
    /// passed construction-time checks.
    pub fn eval(&self, model: &Model) -> Rat {
        self.checked_eval(model).expect("linear eval overflow")
    }

    /// Renders in human-readable form for diagnostics, e.g. `3*x - y + 7`.
    pub fn render(&self) -> String {
        render_linear(&self.terms, self.constant)
    }
}

/// Shared rendering helper (`render` body) kept out of the impl to stay small.
fn render_linear(terms: &[(String, i64)], constant: i64) -> String {
    let mut s = String::new();
    for (v, c) in terms {
        if !s.is_empty() {
            s.push_str(if *c < 0 { " - " } else { " + " });
        } else if *c < 0 {
            s.push('-');
        }
        let mag = c.unsigned_abs();
        if mag != 1 || v.is_empty() {
            s.push_str(&mag.to_string());
            if !v.is_empty() {
                s.push('*');
            }
        }
        s.push_str(v);
    }
    if s.is_empty() {
        // Constant-only expression (or all-zero terms): render the sign
        // attached, e.g. "-3", never " - 3".
        if constant != 0 {
            s.push_str(&constant.to_string());
        } else {
            s.push('0');
        }
    } else if constant != 0 {
        s.push_str(if constant < 0 { " - " } else { " + " });
        s.push_str(&constant.unsigned_abs().to_string());
    }
    s
}

/// One assertion: `lin (rel) 0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint(pub Linear, pub Relation);

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} 0", self.0.render(), self.1)
    }
}

/// An assignment of values to variables.
pub type Model = BTreeMap<String, Rat>;

/// Checks whether `lin (rel) 0` holds under `model`.
pub fn satisfies(constraint: &Constraint, model: &Model) -> bool {
    // Evaluation overflow must never turn into a "violated" verdict: that
    // would let an adversarial model force an UNSAT report. Conservatively
    // treat overflow as satisfied (cannot prove a violation).
    let Some(value) = constraint.0.checked_eval(model) else {
        return true;
    };
    let zero = Rat::ZERO;
    match constraint.1 {
        Relation::Le => match value.checked_cmp(zero) {
            Some(o) => o != Ordering::Greater,
            // Overflow: conservatively treat as satisfied (never a violation
            // we cannot verify).
            None => true,
        },
        Relation::Lt => value.checked_cmp(zero) == Some(Ordering::Less),
        Relation::Ge => match value.checked_cmp(zero) {
            Some(o) => o != Ordering::Less,
            None => true,
        },
        Relation::Gt => value.checked_cmp(zero) == Some(Ordering::Greater),
        Relation::Eq => value == zero,
        Relation::Ne => value != zero,
    }
}

/// Checks every constraint against `model`.
pub fn satisfies_all(constraints: &[Constraint], model: &Model) -> bool {
    constraints.iter().all(|c| satisfies(c, model))
}

/// Negates a constraint into **branches** (a disjunction of conjunctions).
///
/// `premises ⊨ concl  ⟺  premises ∧ branch UNSAT for every branch of
/// negate(concl)`. This mirrors the upstream `negate` helper exactly.
pub fn negate(constraint: &Constraint) -> Vec<Vec<Constraint>> {
    let lin = constraint.0.clone();
    match constraint.1 {
        Relation::Le => vec![vec![Constraint(lin, Relation::Gt)]],
        Relation::Lt => vec![vec![Constraint(lin, Relation::Ge)]],
        Relation::Ge => vec![vec![Constraint(lin, Relation::Lt)]],
        Relation::Gt => vec![vec![Constraint(lin, Relation::Le)]],
        // ¬(a == 0)  ≡  a > 0 ∨ a < 0
        Relation::Eq => vec![
            vec![Constraint(lin.clone(), Relation::Gt)],
            vec![Constraint(lin, Relation::Lt)],
        ],
        // ¬(a != 0)  ≡  a == 0
        Relation::Ne => vec![vec![Constraint(lin, Relation::Eq)]],
    }
}

/// Substitutes `var := expr` in a linear expression. `None` on overflow.
fn substitute_linear(lin: &Linear, var: &str, expr: &Linear) -> Option<Linear> {
    let mut out = Linear::constant_only(lin.constant);
    for (v, c) in &lin.terms {
        if v == var {
            out = out.checked_add(&expr.checked_scale(*c)?)?;
        } else {
            out.add_term(v, *c)?;
        }
    }
    Some(out)
}

/// Rewrites every constraint using the equality `lhs_var == rhs` to eliminate
/// the variable occurrence, mirroring upstream `equality_substitute`.
///
/// Returns `None` when a rewritten coefficient overflows.
pub fn equality_substitute(
    constraints: &[Constraint],
    lhs_var: &str,
    rhs: &Linear,
) -> Option<Vec<Constraint>> {
    constraints
        .iter()
        .map(|Constraint(lin, rel)| Some(Constraint(substitute_linear(lin, lhs_var, rhs)?, *rel)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_arithmetic_merges_terms() {
        let a = Linear::var("x")
            .checked_add(&Linear::constant_only(3))
            .unwrap();
        let b = Linear::var("x").checked_add(&Linear::var("y")).unwrap();
        let sum = a.checked_add(&b).unwrap();
        assert_eq!(
            sum.terms(),
            [("x".to_string(), 2), ("y".to_string(), 1)].as_slice()
        );
        assert_eq!(sum.constant(), 3);

        let diff = a.checked_sub(&b).unwrap();
        assert_eq!(diff.render(), "-y + 3");
    }

    #[test]
    fn arithmetic_overflow_is_reported_not_saturated() {
        // i64::MAX + 1 must not saturate to i64::MAX: that would be a
        // different constraint, not a conservative one.
        let big = Linear::constant_only(i64::MAX);
        assert_eq!(big.checked_add(&Linear::constant_only(1)), None);
        assert_eq!(
            Linear::constant_only(i64::MIN).checked_sub(&Linear::constant_only(1)),
            None
        );
        // Coefficients merge with checked arithmetic too.
        let x2 = Linear::from_parts(vec![("x".into(), i64::MAX)], 0).unwrap();
        assert_eq!(x2.checked_add(&Linear::var("x")), None);
        // Negating i64::MIN on the subtraction path is also an overflow.
        assert_eq!(Linear::constant_only(0).checked_sub(&Linear::constant_only(i64::MIN)), None);
    }

    #[test]
    fn from_parts_merges_duplicate_variables() {
        let l = Linear::from_parts(
            vec![("x".into(), 2), ("y".into(), 1), ("x".into(), -2)],
            4,
        )
        .unwrap();
        // The two `x` entries cancel and drop out entirely.
        assert_eq!(l.terms(), [("y".to_string(), 1)].as_slice());
        assert_eq!(l.constant(), 4);
    }

    #[test]
    fn eval_and_satisfies() {
        // 2*x + 10 <= 0  at x = -5  → 0 <= 0 ✓
        let c = Constraint(
            Linear::var("x")
                .checked_scale(2)
                .unwrap()
                .checked_add(&Linear::constant_only(10))
                .unwrap(),
            Relation::Le,
        );
        let mut m = Model::new();
        m.insert("x".to_string(), Rat::from_i128(-5));
        assert!(satisfies(&c, &m));

        m.insert("x".to_string(), Rat::from_i128(-4));
        assert!(!satisfies(&c, &m));
    }

    #[test]
    fn negate_branches() {
        let eq = Constraint(Linear::var("x"), Relation::Eq);
        assert_eq!(negate(&eq).len(), 2);
        let ne = Constraint(Linear::var("x"), Relation::Ne);
        assert_eq!(
            negate(&ne),
            vec![vec![Constraint(Linear::var("x"), Relation::Eq)]]
        );
    }

    #[test]
    fn substitute_removes_variable() {
        // x == y + 1 ; constraint x >= 0 becomes y + 1 >= 0
        let cs = [Constraint(Linear::var("x"), Relation::Ge)];
        let y_plus_1 = Linear::var("y")
            .checked_add(&Linear::constant_only(1))
            .unwrap();
        let out = equality_substitute(&cs, "x", &y_plus_1).unwrap();
        assert_eq!(out[0].0, y_plus_1);
    }

    #[test]
    fn render_examples() {
        assert_eq!(Linear::constant_only(0).render(), "0");
        assert_eq!(Linear::constant_only(7).render(), "7");
        assert_eq!(Linear::constant_only(-3).render(), "-3");
        assert_eq!(Linear::var("value").render(), "value");
        let l = Linear::var("x")
            .checked_scale(-1)
            .unwrap()
            .checked_add(&Linear::constant_only(-4))
            .unwrap();
        assert_eq!(l.render(), "-x - 4");
    }

    #[test]
    fn eval_overflow_is_conservative_not_panicking() {
        // An adversarial model with an i128::MAX value must not make
        // `satisfies` panic; it must degrade to "satisfied" (cannot verify).
        let c = Constraint(
            Linear::var("x")
                .checked_add(&Linear::constant_only(1))
                .unwrap(),
            Relation::Le,
        );
        let mut m = Model::new();
        m.insert("x".to_string(), Rat::from_i128(i128::MAX));
        assert!(satisfies(&c, &m));
        assert!(satisfies_all(&[c], &m));
        let c2 = Constraint(Linear::var("x"), Relation::Eq); // `x == 0`
                                                             // No overflow here: x == MAX ≠ 0 must be a clean, non-panicking false.
        assert!(!satisfies(&c2, &m));
    }
}
