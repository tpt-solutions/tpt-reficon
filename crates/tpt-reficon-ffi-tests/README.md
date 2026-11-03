# tpt-reficon-ffi-tests

End-to-end tests for the `ffi_wrappers!` macro in
[`tpt-reficon`](https://github.com/tpt-solutions/tpt-reficon). This crate has
no library code. It is not published.

The C side is a Rust function exported under the C symbol name, so the wrapper
links without a C toolchain. The header is the shared sample at
[`crates/tpt-reficon/tests/ffi/payment.h`](https://github.com/tpt-solutions/tpt-reficon/blob/master/crates/tpt-reficon/tests/ffi/payment.h).

## Why it is a separate crate

- The wrappers call C, which needs `unsafe`. The workspace forbids
  `unsafe_code`, so this crate omits `[lints] workspace = true`.
- It has its own empty `[workspace]`. The default workspace build and its CI
  jobs therefore do not need libclang.

## Running

```sh
cargo test --manifest-path crates/tpt-reficon-ffi-tests/Cargo.toml
```

CI runs this in the `ffi` job, which installs libclang first.

## License

MIT OR Apache-2.0, as for the rest of the workspace.
