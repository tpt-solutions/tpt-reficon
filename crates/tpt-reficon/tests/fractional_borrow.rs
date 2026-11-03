//! Tests for `#[fractional_borrow]`. Requires the `unstable` feature.

#![cfg(feature = "unstable")]

use std::sync::Arc;
use std::thread;

use tpt_reficon::prelude::*;

#[fractional_borrow]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Counters {
    hits: u64,
    ok: bool,
    delta: i16,
}

#[test]
fn snapshot_moves_into_shared_cells_and_back() {
    let shared = CountersShared::new(Counters {
        hits: 7,
        ok: true,
        delta: -3,
    });
    assert_eq!(shared.try_read_hits().unwrap().get(), 7);
    assert!(shared.try_read_ok().unwrap().get());
    assert_eq!(shared.try_read_delta().unwrap().get(), -3);
}

#[test]
fn writes_are_per_field() {
    let shared = CountersShared::new(Counters {
        hits: 0,
        ok: false,
        delta: 0,
    });
    let hits = shared.try_write_hits().expect("hits writer");
    hits.set(42);
    // A writer on `hits` does not block readers of `ok`.
    assert!(!shared.try_read_ok().expect("ok reader").get());
    // It does block readers and writers of `hits`.
    assert!(shared.try_read_hits().is_none());
    assert!(shared.try_write_hits().is_none());
    drop(hits);
    assert_eq!(shared.try_read_hits().unwrap().get(), 42);
}

#[test]
fn concurrent_writers_on_one_field_lose_no_updates() {
    let shared = Arc::new(CountersShared::new(Counters {
        hits: 0,
        ok: false,
        delta: 0,
    }));
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let shared = Arc::clone(&shared);
            thread::spawn(move || {
                let mut done = 0;
                while done < 500 {
                    if let Some(w) = shared.try_write_hits() {
                        w.set(w.get() + 1);
                        done += 1;
                    } else {
                        thread::yield_now();
                    }
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(shared.try_read_hits().unwrap().get(), 2000);
}
