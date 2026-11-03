//! `#[refine(proptest)]` gives a refined type an `Arbitrary` impl whose values
//! all satisfy the refinement.

#![allow(clippy::manual_range_contains)] // the predicate is written as a range on purpose

use proptest::prelude::*;
use tpt_reficon::prelude::*;

#[derive(Refine, Debug)]
#[refine(proptest)]
#[requires(value >= 1 && value <= 100)]
pub struct Percent(u8);

proptest! {
    #[test]
    fn generated_values_satisfy_the_refinement(p in any::<Percent>()) {
        prop_assert!((1..=100).contains(&*p));
    }
}
