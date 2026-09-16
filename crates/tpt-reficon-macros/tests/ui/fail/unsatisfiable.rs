use tpt_reficon_macros::Refine;

#[derive(Refine)]
#[requires(value > 0 && value < 0)]
pub struct Impossible(i8);

fn main() {}