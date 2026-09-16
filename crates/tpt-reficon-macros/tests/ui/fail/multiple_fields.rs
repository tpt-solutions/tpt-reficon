use tpt_reficon_macros::Refine;

#[derive(Refine)]
#[requires(value > 0)]
pub struct TwoFields(u8, u16);

fn main() {}