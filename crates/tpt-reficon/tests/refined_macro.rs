//! `refined!` builds a value from a literal, and the refinement is checked.

use tpt_reficon::prelude::*;

#[derive(Refine, Debug)]
#[requires(value >= 1)]
pub struct Port(u16);

/// `%` by a literal over an unsigned type: values that leave remainder 1 mod 4.
#[derive(Refine, Debug)]
#[requires(value % 4 == 1)]
pub struct OneMod4(u8);

#[test]
fn refined_literal_builds_the_value() {
    assert_eq!(*tpt_reficon::refined!(Port, 8080), 8080);
    assert_eq!(*tpt_reficon::refined!(OneMod4, 9), 9);
}

#[test]
fn remainder_refinement_is_checked_at_runtime() {
    assert!(OneMod4::try_new(9).is_ok());
    assert!(OneMod4::try_new(10).is_err());
    assert!(OneMod4::try_new(255).is_err());
    assert!(OneMod4::try_new(253).is_ok());
}
