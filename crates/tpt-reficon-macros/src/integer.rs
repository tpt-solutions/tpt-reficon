//! Single-variable integer-feasibility analysis for refinement predicates.
//!
//! `#[derive(Refine)]` predicates range over exactly one variable, so each DNF
//! branch is a one-dimensional linear system. The general Fourier–Motzkin solver
//! decides such systems over ℚ, which is *sound* for integer types (a rational
//! solution is necessary for an integer one) but *incomplete*: `value > 5 &&
//! value < 6` is ℚ-satisfiable yet admits no integer, and `value > 255` on `u8`
//! is ℚ-satisfiable yet admits no `u8`. This module closes that gap exactly for
//! the single-variable case by reducing every constraint to an exact rational
//! bound on the variable and deciding integer feasibility directly.
//!
//! The analysis is deliberately conservative: anything it cannot decide (an
//! extra variable, or arithmetic overflow while tightening bounds) yields
//! [`IntegerFeasibility::Unknown`], which callers must treat as "keep the
//! runtime check", never as "reject".

use core::cmp::Ordering;

use tpt_reficon_verifier::ir::{satisfies, satisfies_all, Constraint, Linear, Model, Relation};
use tpt_reficon_verifier::rational::Rat;

/// Verdict of the single-variable integer-feasibility analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerFeasibility {
    /// Every integer assignment violates at least one constraint (proven).
    Infeasible,
    /// An integer witness was constructed and verified against the input.
    Feasible,
    /// Not decidable here (extra variable or arithmetic overflow); the caller
    /// must fall back to the rational verdict.
    Unknown,
}

/// Decides whether the conjunction `constraints` has an integer solution for
/// `var`, exactly, when it is a single-variable system.
pub fn analyze(constraints: &[Constraint], var: &str) -> IntegerFeasibility {
    use IntegerFeasibility::{Feasible, Infeasible, Unknown};

    let mut lowers: Vec<(Rat, bool)> = Vec::new();
    let mut uppers: Vec<(Rat, bool)> = Vec::new();
    let mut equality: Option<Rat> = None;
    let mut forbidden: Vec<Rat> = Vec::new();

    for Constraint(lin, rel) in constraints {
        let mut coeff: i64 = 0;
        for (name, c) in &lin.terms {
            if name == var {
                match coeff.checked_add(*c) {
                    Some(x) => coeff = x,
                    None => return Unknown,
                }
            } else {
                return Unknown;
            }
        }

        if coeff == 0 {
            // Independent of the variable: a constant assertion. Decided
            // exactly over the rationals, hence also over the integers.
            let constant_only = Constraint(Linear::constant_only(lin.constant), *rel);
            if !satisfies(&constant_only, &Model::new()) {
                return Infeasible;
            }
            continue;
        }

        // `coeff * var + constant (rel) 0`  ⇔  `var (rel') -constant / coeff`.
        let a = i128::from(coeff);
        let b = i128::from(lin.constant);
        let value = match Rat::new(-b, a) {
            Some(v) => v,
            None => return Unknown,
        };
        let negative = coeff < 0;
        match rel {
            Relation::Le => {
                if negative {
                    lowers.push((value, false));
                } else {
                    uppers.push((value, false));
                }
            }
            Relation::Lt => {
                if negative {
                    lowers.push((value, true));
                } else {
                    uppers.push((value, true));
                }
            }
            Relation::Ge => {
                if negative {
                    uppers.push((value, false));
                } else {
                    lowers.push((value, false));
                }
            }
            Relation::Gt => {
                if negative {
                    uppers.push((value, true));
                } else {
                    lowers.push((value, true));
                }
            }
            Relation::Eq => match equality {
                Some(prev) => {
                    if prev.checked_cmp(value) != Some(Ordering::Equal) {
                        // Two incompatible equalities: no solution at all.
                        return Infeasible;
                    }
                }
                None => equality = Some(value),
            },
            Relation::Ne => forbidden.push(value),
        }
    }

    // An equality pins the variable: feasibility reduces to that exact value.
    if let Some(e) = equality {
        if !e.is_integer() {
            return Infeasible;
        }
        let mut model = Model::new();
        model.insert(var.to_string(), e);
        return if satisfies_all(constraints, &model) {
            Feasible
        } else {
            Infeasible
        };
    }

    let lower = match strongest(lowers, true) {
        Some(b) => b,
        None => return Unknown,
    };
    let upper = match strongest(uppers, false) {
        Some(b) => b,
        None => return Unknown,
    };

    // Integer bounds. Coefficients come from `i64`, so the rounded values fit
    // comfortably in `i128` (no rounding overflow is possible here).
    let lo = lower.map(|(v, strict)| least_integer(v, strict));
    let hi = upper.map(|(v, strict)| greatest_integer(v, strict));

    if let (Some(l), Some(h)) = (lo, hi) {
        if l > h {
            return Infeasible;
        }
    }

    // Search for an integer witness avoiding the finitely many forbidden
    // values. Any `forbidden.len() + 1` consecutive integers contain a free
    // one, so this window suffices whenever the interval is that large.
    let start = lo.or(hi).unwrap_or(0);
    let step: i128 = if lo.is_none() && hi.is_some() { -1 } else { 1 };
    let mut candidate = start;
    for _ in 0..=forbidden.len() {
        if hi.is_some_and(|h| candidate > h) || lo.is_some_and(|l| candidate < l) {
            break;
        }
        let is_forbidden = forbidden
            .iter()
            .any(|f| f.is_integer() && f.numer() == candidate);
        if !is_forbidden {
            let mut model = Model::new();
            model.insert(var.to_string(), Rat::from_i128(candidate));
            if satisfies_all(constraints, &model) {
                return Feasible;
            }
            // A free integer inside the computed bounds failed verification:
            // a rounding edge case. Stay conservative.
            return Unknown;
        }
        match candidate.checked_add(step) {
            Some(next) => candidate = next,
            None => return Unknown,
        }
    }

    // Exhausting a finite interval that is *entirely* forbidden proves
    // infeasibility; an unbounded interval means the window was somehow
    // incomplete, which we refuse to interpret as a proof.
    if lo.is_some() && hi.is_some() {
        Infeasible
    } else {
        Unknown
    }
}

/// Strongest (maximal lower / minimal upper) bound, or `None` on overflow.
fn strongest(bounds: Vec<(Rat, bool)>, want_lower: bool) -> Option<Option<(Rat, bool)>> {
    let mut best: Option<(Rat, bool)> = None;
    for (v, s) in bounds {
        best = Some(match best {
            None => (v, s),
            Some((bv, bs)) => {
                let ord = bv.checked_cmp(v)?;
                let replace = if want_lower {
                    ord == Ordering::Less
                } else {
                    ord == Ordering::Greater
                };
                if replace {
                    (v, s)
                } else if ord == Ordering::Equal {
                    (bv, bs || s)
                } else {
                    (bv, bs)
                }
            }
        });
    }
    Some(best)
}

/// The smallest integer strictly above (`strict`) or at least (`!strict`) `v`.
fn least_integer(v: Rat, strict: bool) -> i128 {
    if strict {
        floor(v) + 1
    } else {
        ceil(v)
    }
}

/// The largest integer strictly below (`strict`) or at most (`!strict`) `v`.
fn greatest_integer(v: Rat, strict: bool) -> i128 {
    if strict {
        ceil(v) - 1
    } else {
        floor(v)
    }
}

/// `floor(n/d)` for `d > 0` (exact).
fn floor(r: Rat) -> i128 {
    r.numer().div_euclid(r.denom())
}

/// `ceil(n/d)` for `d > 0` (exact).
fn ceil(r: Rat) -> i128 {
    let n = r.numer();
    let d = r.denom();
    let f = n.div_euclid(d);
    if n.rem_euclid(d) == 0 {
        f
    } else {
        f + 1
    }
}

#[cfg(test)]
mod tests {
    use super::IntegerFeasibility::{Feasible, Infeasible, Unknown};
    use super::*;

    /// `a * v + b` (as a `Linear`).
    fn lin(a: i64, b: i64) -> Linear {
        Linear {
            terms: if a == 0 {
                Vec::new()
            } else {
                vec![("v".to_string(), a)]
            },
            constant: b,
        }
    }

    fn c(a: i64, b: i64, rel: Relation) -> Constraint {
        Constraint(lin(a, b), rel)
    }

    #[test]
    fn strict_integer_gap_is_infeasible() {
        // 5 < v < 6
        assert_eq!(
            analyze(&[c(1, -5, Relation::Gt), c(1, -6, Relation::Lt)], "v"),
            Infeasible
        );
    }

    #[test]
    fn gap_with_an_integer_is_feasible() {
        // 5 < v < 7  ⇒  v = 6
        assert_eq!(
            analyze(&[c(1, -5, Relation::Gt), c(1, -7, Relation::Lt)], "v"),
            Feasible
        );
    }

    #[test]
    fn exact_equality() {
        assert_eq!(analyze(&[c(1, -5, Relation::Eq)], "v"), Feasible);
        // v == 5 ∧ v != 5
        assert_eq!(
            analyze(&[c(1, -5, Relation::Eq), c(1, -5, Relation::Ne)], "v"),
            Infeasible
        );
    }

    #[test]
    fn forbidden_values_covering_a_finite_interval() {
        // 0 <= v <= 1, v != 0, v != 1
        assert_eq!(
            analyze(
                &[
                    c(1, 0, Relation::Ge),
                    c(1, -1, Relation::Le),
                    c(1, 0, Relation::Ne),
                    c(1, -1, Relation::Ne),
                ],
                "v"
            ),
            Infeasible
        );
        // Removing one exclusion leaves v = 1.
        assert_eq!(
            analyze(
                &[
                    c(1, 0, Relation::Ge),
                    c(1, -1, Relation::Le),
                    c(1, 0, Relation::Ne),
                ],
                "v"
            ),
            Feasible
        );
    }

    #[test]
    fn unbounded_is_feasible() {
        assert_eq!(analyze(&[c(1, 0, Relation::Gt)], "v"), Feasible);
        assert_eq!(analyze(&[], "v"), Feasible);
    }

    #[test]
    fn constant_contradiction_is_infeasible() {
        // -1 >= 0
        assert_eq!(analyze(&[c(0, -1, Relation::Ge)], "v"), Infeasible);
        // -1 <= 0 is fine.
        assert_eq!(analyze(&[c(0, -1, Relation::Le)], "v"), Feasible);
    }

    #[test]
    fn extra_variable_is_unknown() {
        let other = Constraint(
            Linear {
                terms: vec![("w".to_string(), 1)],
                constant: 0,
            },
            Relation::Ge,
        );
        assert_eq!(analyze(&[other], "v"), Unknown);
    }

    #[test]
    fn u8_domain_intersection() {
        // u8 domain (0 <= v <= 255) ∩ (v > 255) is empty.
        assert_eq!(
            analyze(
                &[
                    c(1, 0, Relation::Ge),
                    c(1, -255, Relation::Le),
                    c(1, -255, Relation::Gt),
                ],
                "v"
            ),
            Infeasible
        );
        // (v < 0) on u8 is empty too.
        assert_eq!(
            analyze(
                &[
                    c(1, 0, Relation::Ge),
                    c(1, -255, Relation::Le),
                    c(1, 0, Relation::Lt)
                ],
                "v"
            ),
            Infeasible
        );
        // v <= 255 remains satisfiable.
        assert_eq!(
            analyze(&[c(1, 0, Relation::Ge), c(1, -255, Relation::Le)], "v"),
            Feasible
        );
    }

    fn rel(r: u8) -> Relation {
        match r {
            0 => Relation::Le,
            1 => Relation::Lt,
            2 => Relation::Ge,
            3 => Relation::Gt,
            4 => Relation::Eq,
            _ => Relation::Ne,
        }
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig {
            cases: 512,
            ..proptest::prelude::ProptestConfig::default()
        })]

        /// The critical soundness property: whenever an integer witness exists,
        /// the analysis must never report `Infeasible`.
        #[test]
        fn never_infeasible_when_witness_exists(
            witness in -10i64..=10,
            raw in proptest::collection::vec((-3i64..=3, -10i64..=10, 0u8..6), 0..8),
        ) {
            let mut model = Model::new();
            model.insert("v".to_string(), Rat::from_i128(i128::from(witness)));
            let constraints: Vec<Constraint> = raw
                .iter()
                .map(|&(a, b, r)| Constraint(lin(a, b), rel(r)))
                .filter(|constraint| satisfies(constraint, &model))
                .collect();
            proptest::prop_assert_ne!(
                analyze(&constraints, "v"),
                Infeasible,
                "witness {} satisfies {:?} but the analysis rejected it",
                witness,
                constraints
            );
        }
    }
}
