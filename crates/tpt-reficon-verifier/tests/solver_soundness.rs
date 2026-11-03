//! Property-based soundness tests for the QF_LRA decision procedure.
//!
//! The load-bearing property: `unsat` must **never** claim a system is
//! unsatisfiable when a rational satisfying assignment exists. We hunt for
//! such assignments with a brute-force oracle over a small grid of rational
//! candidates (integers, small denominators, plus the boundary points the
//! constraints themselves define). The oracle is incomplete — it only ever
//! *finds* witnesses — which is exactly what is needed here: any witness it
//! does find must not be contradicted by the solver's UNSAT verdict.
//!
//! Generators deliberately mix loose bounds (wide-open, satisfiable space),
//! tight/contradictory bound pairs (guaranteed UNSAT), equalities, and the
//! occasional `!=`, so both sides of the decision procedure are exercised.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use proptest::prelude::*;

use tpt_reficon_verifier::ir::{negate, satisfies_all};
use tpt_reficon_verifier::rational::Rat;
use tpt_reficon_verifier::{
    decide, entails, model, unsat, Constraint, Linear, Model, Relation, Verdict,
};

const VARS: &[&str] = &["x", "y", "z"];

/// A nonzero coefficient for a variable term.
fn term() -> impl Strategy<Value = (String, i64)> {
    (
        prop::sample::select(VARS).prop_map(str::to_owned),
        -2i64..=2,
    )
        .prop_filter("coeff must be nonzero", |(_, c)| *c != 0)
}

/// A random linear expression over 1..=3 of the three named variables.
fn linear() -> impl Strategy<Value = Linear> {
    (prop::collection::vec(term(), 1..=3), -4i64..=4)
        // Coefficients and sizes are tiny, so the merge cannot overflow.
        .prop_map(|(terms, constant)| Linear::from_parts(terms, constant).unwrap())
        .prop_filter("must mention a variable", |l| !l.terms().is_empty())
}

fn relation() -> impl Strategy<Value = Relation> {
    prop::sample::select(vec![
        Relation::Le,
        Relation::Lt,
        Relation::Ge,
        Relation::Gt,
        Relation::Eq,
        Relation::Ne,
    ])
}

fn constraint() -> impl Strategy<Value = Constraint> {
    (linear(), relation()).prop_map(|(lin, rel)| Constraint(lin, rel))
}

/// A random constraint system. With probability ~1/3 inject the genuinely
/// contradictory pair `x >= b` ∧ `x <= b-1` so UNSAT inputs are well
/// represented (fully random linear real systems are almost always SAT).
fn constraint_system() -> impl Strategy<Value = Vec<Constraint>> {
    (
        prop::bool::ANY,
        prop::collection::vec(constraint(), 0..=5),
        -2i64..=2i64,
    )
        .prop_map(|(inject, mut cs, b)| {
            if inject {
                let x = Linear::var("x");
                let gi = Constraint(
                    x.checked_sub(&Linear::constant_only(b)).unwrap(),
                    Relation::Ge,
                ); // x >= b
                let le = Constraint(
                    x.checked_sub(&Linear::constant_only(b - 1)).unwrap(),
                    Relation::Le,
                ); // x <= b-1
                cs.push(gi);
                cs.push(le);
            }
            cs
        })
}

/// Small rational grid: `k/d` for `k ∈ [-12,12]`, `d ∈ [1,5]`.
fn grid() -> Vec<Rat> {
    let mut out = Vec::new();
    for k in -12i64..=12 {
        for d in 1i64..=5 {
            if let Some(r) = Rat::new(k as i128, d as i128) {
                out.push(r);
            }
        }
    }
    out
}

/// Per-variable candidate values: the grid plus, for every single-term
/// boundary the constraints define, the exact zero-crossing `-constant/coeff`.
fn candidate_values(cs: &[Constraint]) -> BTreeMap<String, Vec<Rat>> {
    let grid: Vec<Rat> = grid();
    let mut per_var: BTreeMap<String, BTreeSet<Rat>> = BTreeMap::new();
    for Constraint(lin, _) in cs {
        for (v, c) in lin.terms() {
            if let Some(boundary) =
                Rat::from_i128(-(lin.constant() as i128)).checked_div(Rat::from_i128(*c as i128))
            {
                per_var.entry(v.clone()).or_default().insert(boundary);
            }
        }
    }
    // Ensure every mentioned variable has a candidate set (grid fallback).
    for Constraint(lin, _) in cs {
        for (v, _) in lin.terms() {
            per_var
                .entry(v.clone())
                .or_default()
                .extend(grid.iter().cloned());
        }
    }
    per_var
        .into_iter()
        .map(|(k, set)| (k, set.into_iter().collect()))
        .collect()
}

/// Depth-first brute-force oracle. Returns a satisfying model when one exists
/// within the candidate grid, `None` otherwise (incomplete by design).
fn brute_witness(cs: &[Constraint]) -> Option<Model> {
    let vars: BTreeSet<String> = cs
        .iter()
        .flat_map(|Constraint(lin, _)| lin.terms().iter().map(|(v, _)| v.clone()))
        .collect();
    if vars.is_empty() {
        return if satisfies_all(cs, &Model::new()) {
            Some(Model::new())
        } else {
            None
        };
    }
    if vars.len() > 2 {
        // keep the oracle cheap; 3-var systems skip brute force
        return None;
    }
    let cands = candidate_values(cs);
    let mut total = 1usize;
    for v in &vars {
        total = total.saturating_mul(cands.get(v).map_or(0, Vec::len));
    }
    if total > 60_000 {
        return None;
    }
    let var_list: Vec<&String> = vars.iter().collect();
    let mut model = Model::new();
    enumerate(&var_list, &cands, 0, &mut model, cs)
}

fn enumerate(
    vars: &[&String],
    cands: &BTreeMap<String, Vec<Rat>>,
    idx: usize,
    model: &mut Model,
    cs: &[Constraint],
) -> Option<Model> {
    if idx == vars.len() {
        return if satisfies_all(cs, model) {
            Some(model.clone())
        } else {
            None
        };
    }
    let v = vars[idx];
    for cand in cands.get(v)?.iter() {
        model.insert(v.clone(), *cand);
        if let Some(found) = enumerate(vars, cands, idx + 1, model, cs) {
            return Some(found);
        }
    }
    model.remove(v);
    None
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// Only peeking at decisions, never exercising a "could not decide" path's
    /// witness: the decision procedure must not call a satisfiable system UNSAT.
    #[test]
    fn never_reports_unsat_when_a_witness_exists(
        cs in constraint_system(),
    ) {
        if let Some(m) = brute_witness(&cs) {
            prop_assert!(satisfies_all(&cs, &m), "oracle found a non-witness");
            prop_assert!(
                !unsat(&cs),
                "solver claims UNSAT for satisfiable system {cs:?} with witness {m:?}"
            );
        }
    }

    /// Any model `model`/`decide` hands back must actually satisfy the input.
    #[test]
    fn returned_models_are_witnesses(cs in constraint_system()) {
        match decide(&cs) {
            Ok(Verdict::Sat(m)) => prop_assert!(satisfies_all(&cs, &m), "invalid model {m:?}"),
            Ok(Verdict::Unsat) => {}
            Ok(Verdict::Unknown(_)) | Err(_) => {}
        }
        if let Some(m) = model(&cs) {
            prop_assert!(satisfies_all(&cs, &m), "invalid model {m:?}");
        }
    }

    /// Entailment must not hold when a countermodel to the conclusion exists.
    #[test]
    fn entails_respects_grid_countermodels(
        cs in constraint_system(),
        c in constraint(),
    ) {
        // `negate` only covers ordering relations; skip `!=`.
        let mut counterfound = false;
        if c.1 != Relation::Ne {
            for branch in negate(&c) {
                let mut combined = cs.clone();
                combined.extend(branch);
                if brute_witness(&combined).is_some() {
                    prop_assert!(!entails(&cs, &c));
                    counterfound = true;
                    break;
                }
            }
        }
        let _ = counterfound;
    }
}
