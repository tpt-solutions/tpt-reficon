# FFI contracts (partial)

> **Status: struct layout proofs are implemented behind the `ffi` feature.**
> Safe wrapper generation and Go integration tests are not built yet. Track
> progress in `todo.md`, Phase 4.

Enable the feature on the facade crate:

```toml
[dependencies]
tpt-reficon = { version = "0.1", features = ["ffi"] }
```

Building with it needs libclang installed, because bindgen loads it at build
time.

```rust
use tpt_reficon::prelude::*;

#[ffi_contract(bridge = "c", header = "payment.h", c_name = "payment")]
#[repr(C)]
pub struct Payment {
    pub id: u64,
    pub amount: i32,
    pub flag: u8,
    pub ptr: *mut f64,
}
```

- `bridge` is `"c"` or `"go"`. Both read a C header. A Go struct only crosses the
  boundary when it is declared as a C struct in the cgo preamble. See
  [`docs/design/go-header-ingestion.md`](../../design/go-header-ingestion.md).
- `header` is relative to the crate's `CARGO_MANIFEST_DIR`.
- `c_name` is optional. Use it when the C struct name differs from the Rust name.

## What is checked

The foreign layout is read from clang's own `sizeof`, `alignof`, and `offsetof`
values, which bindgen emits as layout assertions. The Rust side is computed with
the `repr(C)` algorithm and compared field by field: name, offset, size,
alignment, and total size. A mismatch is a compile error that names the field.

The expansion also emits `const` assertions for `size_of` and `align_of`. Those
run on whatever target the crate is compiled for.

## Limits

- Supported field types: integer and float scalars, `bool`, the `std::os::raw`
  C scalars (except `c_long` and `c_ulong`), raw pointers, and arrays of those.
- Rejected: bitfields, padding, unions, nested structs, and `#[repr(packed)]`.
- Pointer width is taken from the proc macro's host. The `size_of` assertion
  catches a mismatch when cross-compiling.
