use tpt_reficon_macros::contract;

#[contract]
#[ensures(result > 0)]
fn not_an_integer(x: i32) -> bool {
    x > 0
}

fn main() {}
