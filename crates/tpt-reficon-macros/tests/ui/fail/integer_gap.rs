use tpt_reficon_macros::Refine;

// ℚ-satisfiable (5.5) but no integer lies strictly between 5 and 6.
#[derive(Refine)]
#[requires(value > 5 && value < 6)]
pub struct NoInteger(i32);

fn main() {}
