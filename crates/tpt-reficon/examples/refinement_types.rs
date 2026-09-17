//! Runnable tour of `#[derive(Refine)]`.
//!
//! Run with `cargo run --example refinement_types`.
//!
//! The derive discharges `#[requires(...)]` predicates at macro-expansion time:
//! unsatisfiable refinements are compile errors, refinements the type's domain
//! already implies produce zero runtime checks, and everything else becomes an
//! exact, overflow-free runtime check in `new` / `try_new`.
//!
//! Clippy lints tokens embedded in the `#[requires]` attribute itself.
//! Range-style predicates (`x >= -10 && x <= 10`) are linear-comparison
//! conjunctions, so clippy's `manual_range_contains` rewrite to `.contains()`
//! does not apply; only a file-level allow covers attribute-embedded tokens.
#![allow(clippy::manual_range_contains)]

use tpt_reficon::prelude::*;

/// Runtime-checked: `0 < value && value <= 100`.
#[derive(Refine, Debug, Clone, Copy, PartialEq, Eq)]
#[requires(value > 0 && value <= 100)]
pub struct BoundedPercentage(u8);

/// Zero-cost: `u8 <= 255` is implied by the type, so no check is emitted.
#[derive(Refine, Debug, Clone, Copy, PartialEq, Eq)]
#[requires(value <= 255)]
pub struct Byte(u8);

/// Arithmetic that outgrows `u8`; the derive widens the predicate to `i128`
/// so the check never panics in debug or wraps in release.
#[derive(Refine, Debug, Clone, Copy, PartialEq, Eq)]
#[requires(value + 200 < 300)]
pub struct UnderHundred(u8);

/// Named-field form with a custom predicate variable.
#[derive(Refine, Debug, Clone, Copy, PartialEq, Eq)]
#[refine(variable = "x")]
#[requires(x >= -10 && x <= 10)]
pub struct SmallOffset {
    x: i16,
}

fn main() {
    let pct = BoundedPercentage::new(73);
    assert_eq!(*pct, 73);
    assert!(BoundedPercentage::try_new(0).is_err());
    assert!(BoundedPercentage::try_new(101).is_err());
    println!("BoundedPercentage(73) -> {pct:?}");

    // `Byte::try_new` never fails: the predicate is proven elided at
    // compile time, and `try_new` is documented to be infallible.
    let b = Byte::new(200);
    assert!(Byte::try_new(255).is_ok() && Byte::try_new(0).is_ok());
    let _ = b.into_inner();

    // 150 + 200 = 350 >= 300 (evaluated in i128 — no wrap to 194).
    assert!(UnderHundred::try_new(100).is_err());
    assert!(UnderHundred::try_new(99).is_ok());
    println!("UnderHundred: 99 ok, 100 rejected (350 >= 300 in i128)");

    let off = SmallOffset { x: 3 };
    assert_eq!(*off, 3);
    assert!(SmallOffset::try_new(11).is_err());
    println!("SmallOffset(3) -> {off:?}");

    println!("all example assertions passed");
}
