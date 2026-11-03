# Local equivalents of the CI gates. Run `just` with no arguments for the list.

default:
    @just --list

# Everything CI checks, in one go (except the libclang and fuzz jobs).
ci: fmt clippy test doc book go

fmt:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace
    cargo test -p tpt-reficon-core --features unstable
    cargo test -p tpt-reficon --features unstable

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

book:
    CARGO_TARGET_DIR=target/book cargo build -p tpt-reficon --features unstable
    mdbook build docs/user-guide
    mdbook test docs/user-guide -L target/book/debug/deps

go:
    cd crates/tpt-reficon/tests/go/payment && go test ./...

# Needs libclang installed; see CONTRIBUTING.md.
ffi:
    cargo test -p tpt-reficon --features ffi
    cargo test -p tpt-reficon-macros --features ffi-bindgen
    cargo test --manifest-path crates/tpt-reficon-ffi-tests/Cargo.toml

deny:
    cargo deny check --all-features

# Runs the solver fuzz target for 60 seconds. Linux only (libFuzzer on MSVC does not link).
fuzz:
    cd fuzz && cargo fuzz run solver_decide -- -max_total_time=60
