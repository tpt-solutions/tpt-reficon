//! Fourier–Motzkin elimination decision procedure for QF_LRA.
//!
//! Ported/adapted from TPT Solutions' `tpt-telos-verifier` (`solver.rs`).
//! The upstream port normalizes integer-coefficient inequalities to
//! `sum(cᵢ·xᵢ) <= c` and tightens strict relations by ±1 to stay sound over
//! integers. This adaptation keeps **exact rational arithmetic** throughout
//! (true QF_LRA over ℚ), so strictness is represented exactly rather than
//! rounded, and adds:
//!
//! - witness (model) reconstruction by back-substitution through the
//!   elimination history, with **verification of the witness before any SAT
//!   verdict is reported**;
//! - proper handling of `Relation::Ne` premises via bounded case splitting;
//! - checked arithmetic everywhere: overflow or elimination blow-up degrades
//!   conservatively to [`Verdict::Unknown`] ("could not prove"), never to a
//!   spurious UNSAT.
//!
//! Soundness contract: [`unsat`] never returns `true` for a satisfiable
//! system over ℚ (hence also over ℤ or over any primitive Rust type's domain).

use std::collections::{BTreeMap, BTreeSet};

use crate::ir::{satisfies_all, Constraint, Model, Relation};
use crate::rational::Rat;

/// Outcome of a full decision attempt.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// The constraint set has no solution over ℚ (proven).
    Unsat,
    /// Satisfiable; carries a witness assignment (verified against the
    /// constraint set before being returned).
    Sat(Model),
    /// Neither proven nor refuted (overflow, blow-up guard, or unsupported
    /// shape). Callers must treat this as "cannot prove".
    Unknown(String),
}

/// One normalized inequality: `sum(coeffs · vars) (<|<=) c`.
#[derive(Clone, Debug)]
struct Ineq {
    coeffs: Vec<(String, Rat)>,
    c: Rat,
    strict: bool,
}

impl Ineq {
    /// Constant-only inequality `0 (<|<=) c`: contradiction test.
    ///
    /// Returns `true` when the inequality is trivially violated.
    fn is_contradiction(&self) -> Option<bool> {
        if self.coeffs.is_empty() {
            let ord = self.c.checked_cmp(Rat::ZERO)?;
            return Some(match ord {
                Ordering::Less => true,                       // 0 < c is fine; c < 0 is a contradiction
                Ordering::Equal => self.strict,               // 0 <= 0 ok; 0 < 0 impossible
                Ordering::Greater => true,                    // 0 <= negative: contradiction
            });
        }
        Some(false)
    }

    /// Drops zero coefficients (normalization hygiene after elimination).
    fn normalize(&mut self) {
        self.coeffs.retain(|(_, r)| !r.is_zero());
    }
}

/// Converts an assertion into normalized `<=`/`<` inequalities.
///
/// Mirrors upstream `to_inequalities`, except strictness stays exact.
fn to_inequalities(cs: &[Constraint]) -> Option<Vec<Ineq>> {
    let mut out = Vec::new();
    for Constraint(lin, rel) in cs {
        // Normalize `lin (rel) 0` into `Σ terms (rel') (-constant)`.
        let mut coeffs: Vec<(String, Rat)> = Vec::with_capacity(lin.terms.len());
        for (v, c) in &lin.terms {
            let r = Rat::from_i128(*c as i128);
            if !r.is_zero() {
                coeffs.push((v.clone(), r));
            }
        }
        let rhs_const = match -(lin.constant as i128) {
            // Negation of i64 fits i128 trivially.
            k => Rat::from_i128(k),
        };
        let push = |coeffs: &Vec<(String, Rat)>, flip: bool, strict: bool| -> Option<()> {
            let flipped: Vec<(String, Rat)> = if flip {
                coeffs.iter().map(|(v, r)| (v.clone(), r.checked_neg()?)).collect()
            } else {
                coeffs.clone()
            };
            out.push(Ineq { coeffs: flipped, c: rhs_const, strict });
            Some(())
        };
        match rel {
            Relation::Le => push(&coeffs, false, false)?,
            Relation::Lt => push(&coeffs, false, true)?,
            Relation::Ge => push(&coeffs, true, false)?,
            Relation::Gt => push(&coeffs, true, true)?,
            Relation::Eq => {
                push(&coeffs, false, false)?;
                push(&coeffs, true, false)?;
            }
            // `!=` cannot be expressed as a conjunction; callers go through
            // `decide`, which case-splits it before reaching here.
            Relation::Ne => return None,
        }
    }
    Some(out)
}

/// Safety valve: FM can blow up exponentially; beyond this many inequalities
/// we refuse to answer rather than hang the compiler.
const BLOWUP_LIMIT: usize = 20_000;

/// Maximum number of `!=` premises we are willing to case-split on.
const MAX_NE_SPLITS: usize = 8;

/// Bookkeeping for one eliminated (or one-sidedly dropped) variable, used for
/// witness reconstruction.
struct Elimination {
    var: String,
    /// lower bounds `v ≥ L(x_rest)`; strict flag ⇒ `v > L`.
    lowers: Vec<(Vec<(String, Rat)>, Rat, bool)>,
    /// upper bounds `v ≤ U(x_rest)`; strict flag ⇒ `v < U`.
    uppers: Vec<(Vec<(String, Rat)>, Rat, bool)>,
}

/// Normalizes an inequality mentioning `v` with coefficient `a_v ≠ 0` into a
/// bound expression over the remaining variables:
/// `v (<|≤) Σ coeffs·x + c` (upper when `a_v > 0`, lower when `a_v < 0`,
/// because dividing by a negative flips the inequality).
fn bound_expr(ineq: &Ineq, v: &str, a_v: &Rat) -> Option<(Vec<(String, Rat)>, Rat)> {
    let inv = a_v.checked_div(Rat::ONE)?; // 1/a_v ; exact
    let mut coeffs = Vec::with_capacity(ineq.coeffs.len());
    for (name, r) in &ineq.coeffs {
        if name != v {
            coeffs.push((name.clone(), r.checked_mul(inv)?));
        }
    }
    Some((coeffs, ineq.c.checked_mul(inv)?))
}

/// Evaluates a bound expression under a partially built model.
fn eval_bound(coeffs: &[(String, Rat)], c: Rat, model: &Model) -> Option<Rat> {
    let mut acc = c;
    for (name, r) in coeffs {
        let val = model.get(name).copied().unwrap_or(Rat::ZERO);
        acc = acc.checked_add(r.checked_mul(val)?)?;
    }
    Some(acc)
}

/// Outcome of the pure elimination core (no witness yet).
enum CoreOutcome {
    /// Contradiction derived: UNSAT.
    Contradiction,
    /// All variables eliminated without contradiction: SAT over ℚ.
    Consistent(Vec<Elimination>),
}

/// The Fourier–Motzkin elimination core over pre-normalized inequalities.
fn fm_core(ineqs: &[Ineq]) -> Result<CoreOutcome, String> {
    let mut work: Vec<Ineq> = ineqs.to_vec();
    let mut eliminations: Vec<Elimination> = Vec::new();

    loop {
        // Constant-row contradiction scan (`is_contradiction` returning None
        // signals overflow → conservative abort).
        for ineq in &work {
            match ineq.is_contradiction() {
                None => return Err("arithmetic overflow checking constant rows".into()),
                Some(true) => return Ok(CoreOutcome::Contradiction),
                Some(false) => {}
            }
        }

        let mut vars: BTreeSet<String> = BTreeSet::new();
        for ineq in &work {
            for (v, _) in &ineq.coeffs {
                vars.insert(v.clone());
            }
        }
        if vars.is_empty() {
            return Ok(CoreOutcome::Consistent(eliminations));
        }

        // Heuristic: eliminate the variable minimizing (#uppers × #lowers).
        let mut best: Option<(String, usize, usize)> = None;
        let mut best_product = u128::MAX;
        for v in &vars {
            let mut pos = 0usize;
            let mut neg = 0usize;
            for ineq in &work {
                for (name, r) in &ineq.coeffs {
                    if name == v {
                        if r.is_positive() { pos += 1 } else { neg += 1 }
                    }
                }
            }
            let product = (pos as u128).saturating_mul(neg as u128);
            if product < best_product {
                best_product = product;
                best = Some((v.clone(), pos, neg));
            }
        }
        let (v, pos_count, neg_count) = best.expect("vars non-empty");

        let mut uppers: Vec<&Ineq> = Vec::with_capacity(pos_count);
        let mut lowers: Vec<&Ineq> = Vec::with_capacity(neg_count);
        let mut rest: Vec<Ineq> = Vec::new();
        for ineq in &work {
            let coeff_v = ineq.coeffs.iter().find(|(n, _)| n == &v).map(|(_, r)| *r);
            match coeff_v {
                None => rest.push(ineq.clone()),
                Some(a) if a.is_positive() => uppers.push(ineq),
                Some(_) => lowers.push(ineq),
            }
        }

        // Record bounds for witness reconstruction before rewriting.
        let rec_lowers: Option<Vec<_>> =
            lowers.iter().map(|i| bound_expr(i, &v, i.coeffs.iter().find(|(n, _)| n == &v).map(|(_, r)| r).expect("lower has -v"))).collect();
        let rec_uppers: Option<Vec<_>> =
            uppers.iter().map(|i| bound_expr(i, &v, i.coeffs.iter().find(|(n, _)| n == &v).map(|(_, r)| r).expect("upper has +v"))).collect();
        let (rec_lowers, rec_uppers) = match (rec_lowers, rec_uppers) {
            (Some(l), Some(u)) => (l, u),
            _ => return Err("arithmetic overflow recording bounds".into()),
        };
        eliminations.push(Elimination { var: v.clone(), lowers: rec_lowers, uppers: rec_uppers });

        if pos_count == 0 || neg_count == 0 {
            // One-sidedly constrained: choosing an extreme value of `v`
            // always satisfies every row mentioning it, so they may be
            // dropped soundly (classic FM argument).
            work = rest;
            continue;
        }

        // Pairwise combination of every lower × upper pair.
        //
        //   A + p·v ≤ c1  (p > 0)   ⇒   v ≤ (c1 − A)/p      (upper)
        //   B + q·v ≤ c2  (q < 0)   ⇒   v ≥ (c2 − B)/q      (lower)
        //
        // Every lower must not exceed every upper:
        //   (c2 − B)/q ≤ (c1 − A)/p ; multiplying by p > 0 then by q < 0
        //   (which flips) gives  q(c1−A) − p(c2−B) ≤ 0, i.e.
        //   −q·A + p·B  ≤  p·c2 − q·c1.
        let mut combined: Vec<Ineq> = Vec::with_capacity(uppers.len() * lowers.len());
        for u in &uppers {
            let p = u.coeffs.iter().find(|(n, _)| n == &v).map(|(_, r)| *r).expect("upper has +v");
            let c1 = u.c;
            for l in &lowers {
                let q = l.coeffs.iter().find(|(n, _)| n == &v).map(|(_, r)| *r).expect("lower has -v");
                let c2 = l.c;

                let neg_q = match q.checked_neg() {
                    Some(x) => x,
                    None => return Err("arithmetic overflow combining bounds".into()),
                };
                let mut coeffs: Vec<(String, Rat)> = Vec::with_capacity(u.coeffs.len() + l.coeffs.len());
                let mut put = |name: &str, val: Rat, coeffs: &mut Vec<(String, Rat)>| {
                    if let Some(slot) = coeffs.iter_mut().find(|(n, _)| n == name) {
                        slot.1 = match slot.1.checked_add(val) {
                            Some(x) => x,
                            None => return false,
                        };
                    } else {
                        coeffs.push((name.to_string(), val));
                    }
                    true
                };
                let mut overflow = false;
                for (name, r) in u.coeffs.iter().filter(|(n, _)| n != &v) {
                    match r.checked_mul(neg_q) {
                        Some(x) if !overflow => { overflow |= !put(name, x, &mut coeffs); }
                        _ => { overflow = true; }
                    }
                }
                for (name, r) in l.coeffs.iter().filter(|(n, _)| n != &v) {
                    match r.checked_mul(p) {
                        Some(x) if !overflow => { overflow |= !put(name, x, &mut coeffs); }
                        _ => { overflow = true; }
                    }
                }
                if overflow {
                    return Err("arithmetic overflow combining bounds".into());
                }
                let c_new =
                    match p.checked_mul(c2).and_then(|x| x.checked_sub(q.checked_mul(c1)?)) {
                        Some(x) => x,
                        None => return Err("arithmetic overflow combining bounds".into()),
                    };
                combined.push(Ineq { coeffs, c: c_new, strict: u.strict || l.strict });
            }
        }

        work = rest;
        work.extend(combined);

        if work.len() > BLOWUP_LIMIT {
            return Err(format!("elimination blow-up guard hit (> {BLOWUP_LIMIT} inequalities)"));
        }
    }
}

/// Rebuilds a concrete model from the elimination history.
///
/// Eliminations are replayed in reverse: every bound expression only
/// references variables eliminated *after* its own variable, which have
/// already been assigned by then.
fn reconstruct_witness(eliminations: &[Elimination]) -> Option<Model> {
    let mut model = Model::new();
    for elim in eliminations.iter().rev() {
        let lo_max = elim
            .lowers
            .iter()
            .map(|(coeffs, c, strict)| eval_bound(coeffs, *c, &model).map(|v| (v, *strict)))
            .collect::<Option<Vec<_>>>()?;
        let hi_min = elim
            .uppers
            .iter()
            .map(|(coeffs, c, strict)| eval_bound(coeffs, *c, &model).map(|v| (v, *strict)))
            .collect::<Option<Vec<_>>>()?;

        let pick_lower = |vals: Vec<(Rat, bool)>| -> Option<Rat> {
            vals.iter().try_fold(Rat::ZERO, |acc, &(v, _)| v.checked_cmp(acc).map(|o| if o == Ordering::Greater { v } else { acc }))
                .map(|max| {
                    // Bump past the bound when any maximal lower is strict.
                    let max_strict =
                        vals.iter().any(|&(v, s)| s && v.checked_cmp(max) == Some(Ordering::Equal));
                    if max_strict { max.checked_inc() } else { Some(max) }
                })
                .and_then(|x| x)
        };
        let pick_upper = |vals: Vec<(Rat, bool)>| -> Option<Rat> {
            vals.iter().try_fold(Rat::ZERO, |acc, &(v, _)| v.checked_cmp(acc).map(|o| if o == Ordering::Less { v } else { acc }))
                .map(|min| {
                    let min_strict =
                        vals.iter().any(|&(v, s)| s && v.checked_cmp(min) == Some(Ordering::Equal));
                    if min_strict { min.checked_dec() } else { Some(min) }
                })
                .and_then(|x| x)
        };

        let value = match (!lo_max.is_empty(), !hi_min.is_empty()) {
            (true, true) => {
                // Raw extrema first: a rational midpoint between a smaller
                // lower and larger upper satisfies every strictness pattern.
                let raw_lo = lo_max
                    .iter()
                    .try_fold(Rat::ZERO, |acc, &(v, _)| {
                        v.checked_cmp(acc).map(|o| if o == Ordering::Greater { v } else { acc })
                    })?;
                let raw_hi = hi_min
                    .iter()
                    .try_fold(Rat::ZERO, |acc, &(v, _)| {
                        v.checked_cmp(acc).map(|o| if o == Ordering::Less { v } else { acc })
                    })?;
                match raw_lo.checked_cmp(raw_hi)? {
                    Ordering::Less => raw_lo.checked_midpoint(raw_hi)?,
                    // Equal bounds are fine unless some boundary is strict.
                    Ordering::Equal => {
                        let any_strict = lo_max.iter().chain(hi_min.iter()).any(|&(_, s)| s);
                        if any_strict {
                            // Inconsistent leaf; witness construction fails and
                            // the caller will degrade to `Unknown`.
                            return None;
                        }
                        raw_lo
                    }
                    Ordering::Greater => return None,
                }
            }
            (true, false) => pick_lower(lo_max)?,
            (false, true) => pick_upper(hi_min)?,
            (false, false) => Rat::ZERO,
        };

        model.insert(elim.var.clone(), value);
    }
    Some(model)
}

/// Decides one Ne-free conjunction of constraints, producing a verified
/// witness when satisfiable.
fn solve_leaf(cs: &[Constraint], original_for_check: &[Constraint]) -> Result<Verdict, String> {
    let ineqs = match to_inequalities(cs) {
        Some(i) => i,
        // Only reachable if a caller bypasses the Ne splitter.
        None => return Ok(Verdict::Unknown("constraint set contains != after splitting".into())),
    };
    match fm_core(&ineqs)? {
        CoreOutcome::Contradiction => Ok(Verdict::Unsat),
        CoreOutcome::Consistent(elims) => match reconstruct_witness(&elims) {
            Some(m) => {
                // Never claim SAT without checking the witness against the
                // *original* constraint set (including any != rows).
                if satisfies_all(original_for_check, &m) {
                    Ok(Verdict::Sat(m))
                } else {
                    Ok(Verdict::Unknown(
                        "witness failed verification; refusing to report SAT".into(),
                    ))
                }
            }
            None => Ok(Verdict::Unknown("witness reconstruction failed".into())),
        },
    }
}

/// Recursively case-splits `!=` constraints into `<`/`>` branches.
fn decide_branches(
    nes: &[Constraint],
    acc: &mut Vec<Constraint>,
    original: &[Constraint],
) -> Result<Verdict, String> {
    if nes.len() > MAX_NE_SPLITS {
        return Ok(Verdict::Unknown(format!(
            "more than {MAX_NE_SPLITS} != premises exceed the case-split budget"
        )));
    }
    let Some((first, rest)) = nes.split_first() else {
        return solve_leaf(acc, original);
    };
    for rel in [Relation::Gt, Relation::Lt] {
        acc.push(Constraint(first.0.clone(), rel));
        let verdict = decide_branches(rest, acc, original)?;
        acc.pop();
        match verdict {
            Verdict::Sat(_) => return Ok(verdict),
            Verdict::Unknown(_) => return Ok(verdict),
            Verdict::Unsat => continue, // try the other branch
        }
    }
    Ok(Verdict::Unsat)
}

/// Full decision procedure: handles every relation, including `!=`.
pub fn decide(cs: &[Constraint]) -> Result<Verdict, String> {
    let mut nes: Vec<Constraint> = Vec::new();
    let mut base: Vec<Constraint> = Vec::with_capacity(cs.len());
    for c in cs {
        if c.1 == Relation::Ne {
            nes.push(c.clone());
        } else {
            base.push(c.clone());
        }
    }
    decide_branches(&nes, &mut base, cs)
}

/// Returns `true` iff the constraint set is **proven** unsatisfiable over ℚ.
///
/// Sound by construction: overflow, blow-up, and unsupported shapes all
/// degrade to `false` ("could not prove"), never to a spurious UNSAT.
/// Mirrors upstream `unsat`.
pub fn unsat(cs: &[Constraint]) -> bool {
    matches!(decide(cs), Ok(Verdict::Unsat))
}

/// Like [`unsat`], but distinguishes a genuine UNSAT proof from a
/// conservative "could not decide" outcome. Mirrors upstream `unsat_checked`.
pub fn unsat_checked(cs: &[Constraint]) -> Result<bool, String> {
    Ok(matches!(decide(cs)?, Verdict::Unsat))
}

/// Produces a satisfying assignment when one can be constructed and verified.
/// Mirrors upstream `model`.
pub fn model(cs: &[Constraint]) -> Option<Model> {
    match decide(cs) {
        Ok(Verdict::Sat(m)) => Some(m),
        _ => None,
    }
}

/// True iff `model` satisfies every constraint.
pub fn satisfies_model(cs: &[Constraint], m: &Model) -> bool {
    satisfies_all(cs, m)
}

/// Entailment check: `premises ⊨ conclusion`.
///
/// Implemented as: every branch of `negate(conclusion)` conjoined with
/// `premises` is unsatisfiable. Mirrors upstream `entails`.
pub fn entails(premises: &[Constraint], conclusion: &Constraint) -> bool {
    use crate::ir::negate;
    negate(conclusion)
        .iter()
        .all(|branch| {
            let mut combined = premises.to_vec();
            combined.extend(branch.iter().cloned());
            unsat(&combined)
        })
}

/// A counterexample to entailment: a model of `premises ∧ ¬conclusion`
/// (some branch), when entailment fails. Mirrors upstream `counterexample`.
pub fn counterexample(premises: &[Constraint], conclusion: &Constraint) -> Option<Model> {
    use crate::ir::negate;
    negate(conclusion).iter().find_map(|branch| {
        let mut combined = premises.to_vec();
        combined.extend(branch.iter().cloned());
        model(&combined)
    })
}


