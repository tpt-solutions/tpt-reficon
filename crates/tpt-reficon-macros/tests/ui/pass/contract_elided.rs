use tpt_reficon_macros::contract;

// Each of these is proven by the parameter or return-type domain, so the
// expansion contains no runtime check. The unit tests in `contract.rs` check
// that; this file checks that the attribute compiles on each shape.

#[contract]
#[requires(x <= 255)]
fn byte_arg(x: u8) -> u8 {
    x
}

#[contract]
#[ensures(result <= 255)]
fn byte_result(x: i32) -> u8 {
    x as u8
}

#[contract]
#[requires(a > 0)]
#[requires(b > 0)]
#[ensures(result > 0)]
fn sum_positive(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    let _ = byte_arg(1);
    let _ = byte_result(3);
    let _ = sum_positive(1, 2);
}
