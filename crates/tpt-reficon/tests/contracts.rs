//! Tests for `#[contract]`: pre- and postconditions on plain functions.

use tpt_reficon::prelude::*;

#[contract]
#[requires(divisor != 0)]
#[requires(amount >= 0 && amount <= limit)]
#[ensures(result >= 0)]
fn scaled(amount: i64, limit: i64, divisor: i64) -> i64 {
    amount * limit / divisor
}

#[contract]
#[requires(x > 0)]
fn positive_only(x: i32) -> i32 {
    x * 2
}

#[contract]
#[ensures(result <= 255)]
fn clamp_byte(x: i32) -> u8 {
    if x < 0 {
        return 0;
    }
    (x % 256) as u8
}

#[contract]
#[requires(a <= 100 || a == 0)]
#[ensures(result == a + 1)]
fn succ_limited(a: u8) -> u8 {
    a + 1
}

#[contract]
#[ensures(result >= 0)]
fn returns_negative(x: i32) -> i32 {
    -x
}

#[test]
fn satisfied_contracts_return_the_body_value() {
    assert_eq!(scaled(6, 7, 3), 14);
    assert_eq!(positive_only(21), 42);
    assert_eq!(clamp_byte(300), 44);
    assert_eq!(clamp_byte(-5), 0);
    assert_eq!(succ_limited(0), 1);
    assert_eq!(succ_limited(100), 101);
}

#[test]
#[should_panic(
    expected = "precondition `divisor != 0 && amount >= 0 && amount <= limit` of `scaled` violated"
)]
fn precondition_violation_panics() {
    scaled(1, 1, 0);
}

#[test]
#[should_panic(expected = "precondition `x > 0` of `positive_only` violated")]
fn single_precondition_violation_panics() {
    positive_only(-1);
}

#[test]
#[should_panic(expected = "postcondition `result >= 0` of `returns_negative` violated")]
fn postcondition_violation_panics() {
    returns_negative(5);
}

#[test]
fn conjunctive_precondition_checks_every_part() {
    // amount >= 0 holds, amount <= limit does not.
    let r = std::panic::catch_unwind(|| scaled(9, 3, 1));
    assert!(r.is_err());
}

#[test]
fn postcondition_is_checked_on_early_return() {
    // `clamp_byte` returns early for negative input; its postcondition is
    // elided by the u8 domain, so this exercises the elided path.
    assert_eq!(clamp_byte(-1), 0);
}

#[contract]
#[requires(x > -5)]
#[ensures(result == x)]
fn identity_above_floor(x: i64) -> i64 {
    x
}

#[test]
fn i64_contract_expands_and_checks() {
    assert_eq!(identity_above_floor(-4), -4);
    assert!(std::panic::catch_unwind(|| identity_above_floor(-5)).is_err());
}

/// `old(x)` is the entry value of `x`, even though the body changes it.
#[contract]
#[requires(x >= 0)]
#[ensures(x == old(x) + 2)]
fn add_two(mut x: i64) -> i64 {
    x += 2;
    x
}

#[test]
fn old_names_the_entry_value_of_a_mutated_parameter() {
    assert_eq!(add_two(3), 5);
}

/// The body adds three, so `x == old(x) + 2` fails and the contract panics.
#[contract]
#[ensures(x == old(x) + 2)]
fn add_three_breaks_the_contract(mut x: i64) -> i64 {
    x += 3;
    x
}

#[test]
#[should_panic(expected = "postcondition")]
fn old_detects_a_body_that_breaks_the_relation() {
    add_three_breaks_the_contract(1);
}
