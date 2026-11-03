# tpt-reficon-fuzz

[cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets for the
`tpt-reficon-verifier` solver. This crate is not published.

## Targets

| Target | Property |
| --- | --- |
| `solver_decide` | Feeds arbitrary bytes to the decision procedure, as linear constraints over `x`, `y`, and `z`. `decide` must never panic, and any verdict it returns must be internally consistent. |

Soundness against a reference oracle is covered by the proptest suite in
`tpt-reficon-verifier`, not by this fuzzer.

## Running

Requires a nightly toolchain and `cargo-fuzz`.

```sh
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run solver_decide
```

## License

MIT OR Apache-2.0, as for the rest of the workspace.
