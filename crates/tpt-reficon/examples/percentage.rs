//! A percentage in `0..=100`, with a conversion to a fraction.
//!
//! Run with `cargo run -p tpt-reficon --example percentage`.
//!
//! `Percentage` wraps a `u8`, whose domain is `0..=255`, so the constructor
//! checks the upper bound at runtime. `Deref` gives read access to the plain `u8`.

use tpt_reficon::prelude::*;

/// A whole percentage from 0 to 100 inclusive.
#[derive(Refine, Debug, Clone, Copy, PartialEq, Eq)]
#[requires(value <= 100)]
pub struct Percentage(u8);

impl Percentage {
    /// The fraction this percentage stands for, in `0.0..=1.0`.
    pub fn as_fraction(self) -> f64 {
        f64::from(*self) / 100.0
    }
}

/// Applies a percentage discount to a price in cents.
fn discounted(cents: u64, off: Percentage) -> u64 {
    cents - cents * u64::from(*off) / 100
}

fn main() {
    let twenty = Percentage::new(20);
    println!("20% as a fraction: {}", twenty.as_fraction());
    println!("1000 cents after 20% off: {}", discounted(1000, twenty));

    assert!(Percentage::try_new(100).is_ok());
    assert!(Percentage::try_new(101).is_err());
}
