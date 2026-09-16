use tpt_reficon_macros::Refine;

#[derive(Refine)]
#[requires(value * value > 5)]
pub struct Nonlinear(u8);

fn main() {}