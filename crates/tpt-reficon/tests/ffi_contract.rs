//! End-to-end tests for `#[ffi_contract]`: the Rust struct is checked against
//! clang's layout of `tests/ffi/payment.h`. Requires the `ffi` feature.

#![cfg(feature = "ffi")]
#![allow(non_camel_case_types, dead_code)]

use tpt_reficon::prelude::*;

#[ffi_contract(bridge = "c", header = "tests/ffi/payment.h", c_name = "payment")]
#[repr(C)]
pub struct Payment {
    pub id: u64,
    pub amount: i32,
    pub flag: u8,
    pub ptr: *mut f64,
}

#[test]
fn matching_struct_is_accepted() {
    // The layout proof and the const size/align assertions already ran at
    // compile time; this checks the values the header declares.
    assert_eq!(std::mem::size_of::<Payment>(), 24);
    assert_eq!(std::mem::align_of::<Payment>(), 8);
}
