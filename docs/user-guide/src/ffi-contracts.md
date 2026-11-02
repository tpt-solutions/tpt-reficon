# FFI contracts (planned)

> **Status: not implemented.** This chapter describes the intended design. Track
> progress in `todo.md`, Phase 4.

The planned `#[ffi_contract(bridge = "...", header = "...")]` attribute will:

1. Parse a C header through `bindgen`'s AST.
2. Compute the foreign struct layout and compare it with the `repr(C)` struct the
   user wrote. The comparison uses the layout-proof logic in
   `tpt-reficon-verifier`.
3. Generate safe wrapper functions that enforce preconditions before the
   `unsafe` FFI call.

Go interop is under investigation. Go has no native C headers, so the options are
cgo-generated headers or a Go-side codegen helper.

Until this lands, the `Refine` derive is the only shipped piece of the contract
machinery.
