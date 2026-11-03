//! Regression tests for the refinement derive's runtime checks.

#![allow(clippy::bool_comparison)] // `value == true` is the point of one test

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

/// `u128` values above `i128::MAX` used to wrap negative when the predicate
/// was widened with `as i128`, so `value < 1000` accepted `u128::MAX`.
#[derive(Refine, Debug)]
#[requires(value < 1000)]
pub struct SmallWide(u128);

#[test]
fn u128_above_i128_max_is_rejected_not_wrapped() {
    assert!(SmallWide::try_new(999).is_ok());
    assert!(SmallWide::try_new(1000).is_err());
    assert!(SmallWide::try_new(u128::MAX).is_err());
}

/// Negation pushed inward: outside the open range `(10, 20)`.
#[derive(Refine, Debug)]
#[requires(!(value > 10 && value < 20))]
pub struct OutsideTeens(u8);

#[test]
fn negated_range_is_checked_at_runtime() {
    assert!(OutsideTeens::try_new(5).is_ok());
    assert!(OutsideTeens::try_new(10).is_ok());
    assert!(OutsideTeens::try_new(15).is_err());
    assert!(OutsideTeens::try_new(20).is_ok());
}

/// Multiplication by a constant stays linear: `2 * value < 10`.
#[derive(Refine, Debug)]
#[requires(value * 2 < 10)]
pub struct DoubleBelowTen(u8);

#[test]
fn constant_multiplication_is_checked_at_runtime() {
    assert!(DoubleBelowTen::try_new(4).is_ok());
    assert!(DoubleBelowTen::try_new(5).is_err());
}

/// The refined-to-plain conversion is total, so `From` needs no check.
#[derive(Refine, Debug)]
#[requires(value >= 1)]
pub struct Ticket(u32);

#[test]
fn refined_value_converts_into_its_inner_type() {
    let plain: u32 = Ticket::new(7).into();
    assert_eq!(plain, 7);
}

#[test]
fn refinement_error_reports_the_rejected_value() {
    let err = Ticket::try_new(0).unwrap_err();
    assert_eq!(err.value(), Some("0"));
    assert!(err.to_string().ends_with("(got 0)"), "{err}");
}

/// `bool` is the integers 0 and 1, so a comparison with `true` is a check.
#[derive(Refine, Debug)]
#[requires(value == true)]
pub struct Enabled(bool);

#[test]
fn bool_comparison_with_a_literal_is_checked() {
    assert!(Enabled::try_new(true).is_ok());
    assert!(Enabled::try_new(false).is_err());
}

/// Parsing goes through the inner type and then the refinement.
#[derive(Refine, Debug)]
#[requires(value > 0)]
pub struct Count(u32);

#[test]
fn refined_values_parse_display_and_borrow_as_their_inner_type() {
    let n: Count = "42".parse().unwrap();
    assert_eq!(n.to_string(), "42");
    assert_eq!(*AsRef::<u32>::as_ref(&n), 42);
    assert!("0".parse::<Count>().is_err());
    let garbage = "x".parse::<Count>().unwrap_err();
    assert!(garbage.to_string().contains("does not parse"), "{garbage}");
}
