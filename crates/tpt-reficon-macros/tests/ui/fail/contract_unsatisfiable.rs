use tpt_reficon_macros::contract;

#[contract]
#[requires(x > 5 && x < 3)]
fn never_callable(x: i64) -> i64 {
    x
}

fn main() {}
