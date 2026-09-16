use tpt_reficon_macros::Refine;

#[derive(Refine)]
#[requires(other > 0)]
pub struct HasUnknownVar(u8);

fn main() {}