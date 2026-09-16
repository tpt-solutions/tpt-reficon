use tpt_reficon_core::Refine;
use tpt_reficon_macros::Refine;

#[derive(Refine)]
#[requires(0 < value && value <= 100)]
pub struct BoundedPercentage {
    value: u8,
}

fn main() {
    // Not implied by the u8 domain (0 is a valid u8), so it is runtime-checked.
    let ok = BoundedPercentage::try_new(100).unwrap();
    assert_eq!(ok.into_inner(), 100);
    assert!(BoundedPercentage::try_new(0).is_err());
}