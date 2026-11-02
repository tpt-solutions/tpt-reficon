//! End-to-end tests for `ffi_wrappers!`.
//!
//! The C side is a Rust function exported under the C symbol name, so the
//! wrapper links without a C toolchain. The header is the shared sample in the
//! facade crate, which declares `int32_t process_payment(int32_t amount);`.

use tpt_reficon::prelude::*;

mod c_side {
    /// Exported under the C name the header declares.
    #[export_name = "process_payment"]
    pub extern "C" fn process_payment_impl(amount: i32) -> i32 {
        amount * 2
    }
}

ffi_wrappers! {
    header = "../tpt-reficon/tests/ffi/payment.h";
    bridge = "c";

    /// Doubles a payment amount in C.
    #[requires(amount > 0)]
    #[ensures(result > amount)]
    pub fn process_payment(amount: i32) -> i32;
}

#[test]
fn wrapper_calls_the_c_function_when_contracts_hold() {
    assert_eq!(process_payment(21), 42);
}

#[test]
#[should_panic(expected = "precondition `amount > 0` of `process_payment` violated")]
fn precondition_is_checked_before_the_c_call() {
    process_payment(0);
}

#[test]
fn the_c_side_is_linked() {
    // Calls the export directly, bypassing the wrapper, to confirm the symbol.
    assert_eq!(c_side::process_payment_impl(4), 8);
}
