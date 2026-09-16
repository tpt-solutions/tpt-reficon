use tpt_reficon_macros::Refine;

#[derive(Refine)]
#[requires(value > 0)]
pub struct Generic<T>(T);

fn main() {}