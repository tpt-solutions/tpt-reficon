//! Money held as whole cents, never negative.
//!
//! Run with `cargo run -p tpt-reficon --example money_cents`.
//!
//! The refinement guarantees the sign. Overflow is a separate matter: the
//! `checked_add` below returns `None` instead of wrapping, so a total never
//! silently goes wrong.

use tpt_reficon::prelude::*;

/// An amount of money in cents, zero or more.
#[derive(Refine, Debug, Clone, Copy, PartialEq, Eq)]
#[requires(value >= 0)]
pub struct Cents(i64);

/// Adds two amounts, or `None` if the sum overflows `i64`.
fn total(a: Cents, b: Cents) -> Option<Cents> {
    let sum = (*a).checked_add(*b)?;
    // The sum of two non-negative values is non-negative, so this cannot fail.
    Some(Cents::new(sum))
}

fn main() {
    let coffee = Cents::new(350);
    let tea = Cents::new(275);
    if let Some(sum) = total(coffee, tea) {
        println!("coffee and tea cost {} cents", sum.get());
    }

    assert!(Cents::try_new(-1).is_err());
    assert!(total(Cents::new(i64::MAX), Cents::new(1)).is_none());
}
