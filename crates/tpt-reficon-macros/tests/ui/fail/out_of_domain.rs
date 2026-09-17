use tpt_reficon_macros::Refine;

// ℚ-satisfiable but no `u8` exceeds 255.
#[derive(Refine)]
#[requires(value > 255)]
pub struct TooBigForU8(u8);

fn main() {}
