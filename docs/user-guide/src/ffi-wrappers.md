# FFI wrappers

`ffi_wrappers!` declares safe Rust functions around C functions in a header.

```rust
use tpt_reficon::prelude::*;

ffi_wrappers! {
    header = "payment.h";
    bridge = "c";

    /// Doubles a payment amount in C.
    #[requires(amount > 0)]
    #[ensures(result > amount)]
    pub fn process_payment(amount: i32) -> i32;
}

// `process_payment` is now safe to call.
let doubled = process_payment(21);
```

- Each declared signature is checked against the header. A different parameter
  count, type, or return type is a compile error. `i32` and `c_int` are the same
  type here, and pointer mutability counts.
- `#[requires]` and `#[ensures]` are checked before the unsafe call, and the rules
  are the ones in [Function contracts](function-contracts.md).
- The unsafe call is confined to the generated wrapper.

The crate that uses the macro must allow `unsafe_code`. A crate that forbids it
cannot use the macro. The design is in
[`docs/design/ffi-wrappers.md`](../../design/ffi-wrappers.md).
