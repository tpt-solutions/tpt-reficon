//! Pre- and postconditions on plain functions with `#[contract]`.
//!
//! Run with `cargo run -p tpt-reficon --example contract_basics`.
//!
//! `#[requires]` is checked on entry and `#[ensures]` on the return value. A
//! check the parameter or return domain already proves is compiled out, so
//! `halve` below pays for its precondition only where it is not provable.

use tpt_reficon::prelude::*;

/// Divides a total into `parts` equal shares. The divisor must be positive.
#[contract]
#[requires(parts > 0)]
#[ensures(result >= 0)]
fn share(total: i64, parts: i64) -> i64 {
    total / parts
}

/// Halves a byte. The result is `u8`, so the `ensures` below is proven by the
/// return domain and is compiled out.
#[contract]
#[ensures(result <= 255)]
fn halve(value: u8) -> u8 {
    value / 2
}

/// Clamps a reading to the sensor's range.
#[contract]
#[requires(lo <= hi)]
#[ensures(result >= lo && result <= hi)]
fn clamp(value: i64, lo: i64, hi: i64) -> i64 {
    value.clamp(lo, hi)
}

fn main() {
    println!("share 10 across 4: {}", share(10, 4));
    println!("halve 201: {}", halve(201));
    println!("clamp 50 into 0..=20: {}", clamp(50, 0, 20));
}
