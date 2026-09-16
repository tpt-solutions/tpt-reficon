use tpt_reficon_core::Refine;
use tpt_reficon_macros::Refine;

#[derive(Refine)]
#[requires(value <= 255)]
pub struct Byte(u8);

fn main() {
    // `value <= 255` is implied by the u8 domain, so the runtime check is
    // elided (proven at compile time) and `new` never panics.
    let b = Byte::new(200);
    assert_eq!(*b, 200);
    assert_eq!(Byte::try_new(255).unwrap().into_inner(), 255);
}