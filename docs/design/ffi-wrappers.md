# Safe FFI wrappers (`ffi_wrappers!`, Phase 4)

Status: implemented for scalar and pointer signatures, behind the `ffi` feature.

## Surface

```rust
ffi_wrappers! {
    header = "payment.h";
    bridge = "c";

    #[requires(amount > 0)]
    #[ensures(result > amount)]
    pub fn process_payment(amount: i32) -> i32;
}
```

Each declaration yields a safe function with the declared name, plus a private
`extern "C"` declaration of the raw symbol.

## Decisions

1. **The signature is checked against the header.** The function must appear in
   bindgen's output for `header`. Parameter count, each parameter type, and the
   return type must match after canonicalising the `c_*` aliases to Rust
   primitives. Pointer mutability is part of the comparison. A mismatch is a
   compile error, so a wrong `i64` for an `int32_t` cannot reach the call.
2. **Preconditions go through `#[contract]`.** The wrapper carries the user's
   `#[requires]` and `#[ensures]`, and the contract expansion checks them before
   the unsafe call. Parameter-domain elision applies, so `#[requires(x <= 255)]`
   on a `u8` costs nothing. Contracts follow the rules in
   `function-contracts.md`, and they require integer parameters.
3. **The unsafe call is confined.** The raw declaration and the single unsafe
   block are generated. The wrapper carries `#[allow(unsafe_code)]`. A crate that
   forbids `unsafe_code` cannot use the macro, because the lint level cannot be
   overridden there.
4. **Link names are the C names.** `#[link_name]` links the private raw symbol
   to the function name in the header.
5. **Only the signature is checked.** The macro does not check that the C
   function's behaviour matches the contract. That is the user's claim, and the
   preconditions are the Rust side of it.

## Limits

- No methods, generics, variadic functions, or `extern` ABI overrides.
- Types outside scalars, raw pointers, `c_void`, and unit are rejected.
- Function-pointer and nullable-pointer parameters are not supported.
- The Go bridge takes the same path, through the `c-shared` header.

## Testing

Unit tests in `tpt-reficon-macros` cover each signature mismatch. The end-to-end
test is in `crates/tpt-reficon-ffi-tests`, which is a standalone crate outside the
workspace lint set. It exports a Rust function under the C symbol name, so the
call links without a C toolchain. It runs in the `ffi` CI job.
