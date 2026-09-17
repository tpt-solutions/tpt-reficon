use tpt_reficon_macros::Refine;

// ℚ-satisfiable *and* integer-satisfiable: must still compile.
#[derive(Refine)]
#[requires(value > 5 && value < 7)] // witness: 6
pub struct Sixish(i32);

// Tight but non-empty domain intersection: {255}.
#[derive(Refine)]
#[requires(value >= 255)] // witness: 255
pub struct MaxByte(u8);

// A disjunction with one integer-feasible alternative.
#[derive(Refine)]
#[requires(value > 5 && value < 6 || value == 9)] // witness: 9
pub struct GapOrNine(i32);

fn main() {}
