//! A TCP/UDP port: an integer in `1..=65535`, with the zero port rejected.
//!
//! Run with `cargo run -p tpt-reficon --example port`.
//!
//! The `u16` domain already bounds the value to `0..=65535`, so the derive only
//! checks the lower bound. Constructing a literal such as `Port::new(8080)` is
//! checked at runtime, while `try_new` returns an error instead of panicking.

use tpt_reficon::prelude::*;

/// A port number that is never zero.
#[derive(Refine, Debug, Clone, Copy, PartialEq, Eq)]
#[requires(value >= 1)]
pub struct Port(u16);

fn main() {
    let http = Port::new(8080);
    println!("listening on port {}", *http);

    for candidate in [0u16, 443, 65535] {
        match Port::try_new(candidate) {
            Ok(port) => println!("accepted {}", port.get()),
            Err(err) => println!("rejected {candidate}: {err}"),
        }
    }

    assert!(Port::try_new(0).is_err());
    assert!(Port::try_new(u16::MAX).is_ok());
}
