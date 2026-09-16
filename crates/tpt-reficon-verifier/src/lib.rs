//! `tpt-reficon-verifier` — constraint IR, QF_LRA decision procedure, and
//! memory-layout/alignment proving.
//!
//! The Fourier–Motzkin elimination engine over exact rational arithmetic is
//! ported/adapted from TPT Solutions' `tpt-telos-verifier`. It decides
//! quantifier-free linear real arithmetic (QF_LRA): given a set of linear
//! [`Constraint`]s it returns a verified witness when satisfiable, proves
//! unsatisfiability, or reports "could not prove" ([`Verdict::Unknown`]) on
//! overflow / blow-up / unsupported shapes — never a spurious verdict.
//!
//! Two higher-level entry points:
//!
//! - [`solver::entails`] / [`solver::counterexample`] — discharge
//!   `requires`/`ensures` conditions: `premises ⊨ conclusion`.
//! - [`layout`] — prove that a Rust `repr(C)` struct mirrors a foreign
//!   struct's byte layout (offsets, alignment, size), the foundation for FFI
//!   contract checking.
//!
//! Soundness contract: [`solver::unsat`] never returns `true` for a
//! satisfiable system over ℚ.
//!
//! See the crate-level `CHANGELOG.md`/"NOTICE" at the workspace root for
//! attribution of the ported code.

pub mod ir;
pub mod layout;
pub mod rational;
pub mod solver;

pub use ir::{Constraint, Linear, Model, Relation};
pub use solver::{decide, entails, model, unsat, unsat_checked, Verdict};
