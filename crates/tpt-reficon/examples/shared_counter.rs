//! Lock-free shared state with `#[fractional_borrow]`.
//!
//! Run with `cargo run -p tpt-reficon --example shared_counter --features unstable`.
//!
//! This is a prototype behind the `unstable` feature. Each field has its own
//! permission. A writer on `hits` does not block a reader of `ok`.

use tpt_reficon::prelude::*;

#[fractional_borrow]
#[derive(Debug, Clone, Copy)]
pub struct Counters {
    hits: u64,
    ok: bool,
}

fn main() {
    let shared = CountersShared::new(Counters { hits: 0, ok: true });

    if let Some(w) = shared.try_write_hits() {
        w.set(w.get() + 1);
    }
    if let Some(r) = shared.try_read_ok() {
        println!("ok flag: {}", r.get());
    }
    println!("hits: {}", shared.try_read_hits().map_or(0, |r| r.get()));
}
