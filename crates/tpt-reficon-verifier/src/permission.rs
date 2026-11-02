//! Fractional-permission non-interference proofs (research prototype).
//!
//! A shared resource carries a unit of permission, split into fractions
//! `q_i ∈ (0, 1]` held by concurrent accessors. The pool invariant is
//! `Σ q_i ≤ 1`. A writer must hold the whole unit (`q_w = 1`), so a writer may
//! not coexist with any other holder. Readers need any positive fraction.
//!
//! [`check_configuration`] encodes a proposed set of holders as a QF_LRA
//! system and asks the solver whether some assignment of fractions satisfies
//! it. If not, the configuration is a proven interference: no permission
//! split can grant those accesses at once.
//!
//! This is a prototype behind the `unstable` feature of the facade crate. The
//! encoding is a deliberately small slice of the fractional-permission model
//! (Boyland 2003); see `docs/design/fractional-borrows.md`.

use crate::ir::{Constraint, Linear, Relation};
use crate::solver::{decide, Verdict};

/// The access a holder requests on the shared resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Shared read access: any positive fraction.
    Read,
    /// Exclusive write access: the full unit.
    Write,
}

/// Why a proposed set of holders was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interference {
    /// The solver proved no fraction split satisfies the configuration.
    Proven,
    /// The solver could not decide either way; treated as a rejection.
    Undecided(String),
}

/// Checks that some fraction split grants every access in `holders` at once.
///
/// Returns `Ok(())` when the solver finds a satisfying assignment, and an
/// [`Interference`] otherwise. An empty configuration is trivially `Ok`.
pub fn check_configuration(holders: &[Access]) -> Result<(), Interference> {
    if holders.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = (0..holders.len()).map(|i| format!("q{i}")).collect();

    let mut cs = Vec::with_capacity(holders.len() * 2 + 1);

    // Pool invariant: Σ q_i − 1 ≤ 0.
    let mut total = Linear::constant_only(-1);
    for name in &names {
        total = total.add(&Linear::var(name.clone()));
    }
    cs.push(Constraint(total, Relation::Le));

    for (name, access) in names.iter().zip(holders) {
        // Every holder has a strictly positive fraction: q_i > 0.
        cs.push(Constraint(Linear::var(name.clone()), Relation::Gt));
        if *access == Access::Write {
            // A writer holds the whole unit: q_i − 1 = 0.
            cs.push(Constraint(
                Linear::var(name.clone()).add(&Linear::constant_only(-1)),
                Relation::Eq,
            ));
        }
    }

    match decide(&cs) {
        Ok(Verdict::Sat(_)) => Ok(()),
        Ok(Verdict::Unsat) => Err(Interference::Proven),
        Ok(Verdict::Unknown(reason)) => Err(Interference::Undecided(reason)),
        Err(reason) => Err(Interference::Undecided(reason)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_alone_coexist() {
        assert_eq!(check_configuration(&[Access::Read; 5]), Ok(()));
    }

    #[test]
    fn single_writer_is_admitted() {
        assert_eq!(check_configuration(&[Access::Write]), Ok(()));
    }

    #[test]
    fn writer_excludes_any_reader() {
        assert_eq!(
            check_configuration(&[Access::Write, Access::Read]),
            Err(Interference::Proven)
        );
        assert_eq!(
            check_configuration(&[Access::Read, Access::Write]),
            Err(Interference::Proven)
        );
    }

    #[test]
    fn two_writers_conflict() {
        assert_eq!(
            check_configuration(&[Access::Write, Access::Write]),
            Err(Interference::Proven)
        );
    }

    #[test]
    fn empty_configuration_is_trivially_ok() {
        assert_eq!(check_configuration(&[]), Ok(()));
    }
}
