use tpt_reficon_macros::Refine;

#[derive(Refine)]
#[requires((value > 5 && value < 3) || (value > 9 && value < 4))]
pub struct NoWhere(i8);

fn main() {}
