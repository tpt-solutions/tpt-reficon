//! Regression tests for the refinement derive's runtime checks.

use tpt_reficon::prelude::*;

/// Two `#[requires]` attributes, the second containing `||`. Before the fix,
/// the runtime check parsed as `(value < 10 && value == 0) || value == 42` and
/// accepted 42, which violates `value < 10`.
#[derive(Refine, Debug)]
#[requires(value < 10)]
#[requires(value == 0 || value == 42)]
pub struct DisjunctionAfterConjunct(u8);

#[test]
fn disjunction_does_not_leak_past_a_conjunct() {
    assert!(DisjunctionAfterConjunct::try_new(0).is_ok());
    assert!(DisjunctionAfterConjunct::try_new(42).is_err());
    assert!(DisjunctionAfterConjunct::try_new(7).is_err());
}

/// `i64` once panicked during expansion: the domain's lower bound negated
/// `i64::MIN`. Any refinement on an `i64` exercises that path.
#[derive(Refine, Debug)]
#[requires(value > -5)]
pub struct SignedFloor(i64);

#[test]
fn i64_refinement_expands_and_checks() {
    assert!(SignedFloor::try_new(-4).is_ok());
    assert!(SignedFloor::try_new(-5).is_err());
    assert!(SignedFloor::try_new(i64::MAX).is_ok());
}
