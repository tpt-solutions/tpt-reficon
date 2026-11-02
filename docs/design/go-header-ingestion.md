# Go header ingestion (Phase 4 investigation)

Question from `todo.md`: Go has no native C headers. Should `#[ffi_contract]`
ingest cgo-generated headers, or should a Go-side codegen helper produce the
layout?

## What was checked

Probe with Go 1.25.5 on Windows, `-buildmode=c-shared`:

- `go build -buildmode=c-shared` emits a `.h` containing only the **exported**
  functions (`//export`) and the types they mention, using the C spellings
  (`GoUint64`, `GoInt32`, `C.int`, …). Go-only `struct` types such as
  `type Payment struct { ID uint64; ... }` do **not** appear in that header.
- `go tool cgo -godefs` prints the Go-side declarations for a file. It rewrites
  the file as Go, it does not emit C layout.

So a Go struct has no C-visible layout unless the user declares it in C.

## Decision

Use the **C header as the single layout source**, for both C and Go libraries:

1. For Go, the struct that crosses the boundary is declared in the cgo preamble
   (`import "C"` comment block) as a C `struct`, and exported functions take
   `C.struct_*` pointers. The `-buildmode=c-shared` header then carries that
   struct, and it goes through the same ingestion path as a hand-written C
   header.
2. A Go-side codegen helper is **not** built. It would reproduce cgo's layout
   rules with a second implementation that can drift from the Go toolchain.

Trade-off: Go structs that are not declared in the preamble cannot be checked.
`#[ffi_contract]` must reject such a bridge with a clear diagnostic rather than
guessing a layout.

## Follow-up

- Integration test (Phase 4, last item) builds a small Go `c-shared` library
  with a preamble struct and checks it against a matching `repr(C)` struct.
- The C-side integration test uses a checked-in header under the test crate's
  `tests/ffi/` directory, so CI does not need Go installed for the default job.
