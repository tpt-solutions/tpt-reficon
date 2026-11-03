//! A quantity of at least one, used as a divisor and a multiplier.
//!
//! Run with `cargo run -p tpt-reficon --example non_zero_quantity`.
//!
//! Holding a `NonZeroQuantity` lets a function skip the zero check it would
//! otherwise need before dividing. The check lives once, in the constructor.

use tpt_reficon::prelude::*;

/// A count of items, never zero.
#[derive(Refine, Debug, Clone, Copy, PartialEq, Eq)]
#[requires(value > 0)]
pub struct NonZeroQuantity(u32);

/// Splits `total` evenly across `parts`. The divisor cannot be zero, because
/// the type excludes it.
fn per_part(total: u32, parts: NonZeroQuantity) -> u32 {
    total / *parts
}

fn main() {
    let parts = NonZeroQuantity::new(4);
    println!("12 items across 4 parts: {} each", per_part(12, parts));

    match NonZeroQuantity::try_new(0) {
        Ok(_) => unreachable!("zero is excluded by the refinement"),
        Err(err) => println!("refused a zero quantity: {err}"),
    }
}
