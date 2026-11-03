//! Lock-free fractional-permission accessor for `u64` values (prototype).
//!
//! The permission to access a [`SharedU64`] is a pool of [`WHOLE`] units.
//! A reader takes one unit; a writer takes the whole pool. Handles return their
//! units on drop. Because a writer must hold every unit, a writer can never
//! coexist with a reader. This is the same model that
//! `tpt_reficon_verifier::permission::check_configuration` checks statically.
//!
//! Acquisition is a compare-and-swap loop and never blocks. The value itself
//! is an [`AtomicU64`], so no `unsafe` is needed. The workspace forbids
//! `unsafe_code`, which is why the accessor is restricted to atomic-sized
//! integers.
//!
//! This module is gated behind the `unstable` feature. Its API may change.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Units in the whole permission. A writer takes all of them at once.
pub const WHOLE: u32 = u32::MAX;

/// A `u64` shared between threads with fractional read and write permissions.
#[derive(Debug)]
pub struct SharedU64 {
    /// Units of permission not currently held by any handle.
    free: AtomicU32,
    value: AtomicU64,
}

impl SharedU64 {
    /// Creates a shared cell holding `value`, with the whole permission free.
    pub const fn new(value: u64) -> Self {
        SharedU64 {
            free: AtomicU32::new(WHOLE),
            value: AtomicU64::new(value),
        }
    }

    /// Takes one unit of read permission, or `None` if a writer holds the cell
    /// or every unit is taken. Never blocks.
    pub fn try_read(&self) -> Option<ReadHandle<'_>> {
        let mut current = self.free.load(Ordering::Acquire);
        loop {
            if current == 0 {
                return None;
            }
            match self.free.compare_exchange_weak(
                current,
                current - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(ReadHandle { cell: self }),
                Err(actual) => current = actual,
            }
        }
    }

    /// Takes the whole permission, or `None` if any unit is held. Never blocks.
    pub fn try_write(&self) -> Option<WriteHandle<'_>> {
        self.free
            .compare_exchange(WHOLE, 0, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| WriteHandle { cell: self })
    }

    /// Units currently free. `WHOLE` means no handle is live.
    pub fn free_units(&self) -> u32 {
        self.free.load(Ordering::Acquire)
    }
}

/// One unit of read permission. Returns it on drop.
#[derive(Debug)]
pub struct ReadHandle<'a> {
    cell: &'a SharedU64,
}

impl ReadHandle<'_> {
    /// The current value.
    pub fn get(&self) -> u64 {
        self.cell.value.load(Ordering::Acquire)
    }
}

impl Drop for ReadHandle<'_> {
    fn drop(&mut self) {
        // Held units never exceed WHOLE, so this cannot overflow.
        self.cell.free.fetch_add(1, Ordering::AcqRel);
    }
}

/// The whole permission. Returns it on drop.
#[derive(Debug)]
pub struct WriteHandle<'a> {
    cell: &'a SharedU64,
}

impl WriteHandle<'_> {
    /// The current value.
    pub fn get(&self) -> u64 {
        self.cell.value.load(Ordering::Acquire)
    }

    /// Replaces the value. Exclusive, so no reader can observe a torn update.
    pub fn set(&self, value: u64) {
        self.cell.value.store(value, Ordering::Release);
    }
}

impl Drop for WriteHandle<'_> {
    fn drop(&mut self) {
        // While the writer is live no reader exists, so every unit is free
        // from the writer's point of view. Restoring the whole pool is safe.
        self.cell.free.store(WHOLE, Ordering::Release);
    }
}

/// A type that fits in the 64-bit cell behind [`SharedU64`].
///
/// Implemented for the primitive integers and `bool`. Signed values round-trip
/// through `u64` because the casts sign-extend and truncate consistently.
pub trait Bits: Copy {
    /// The value's bits, zero-extended to 64 bits.
    fn to_bits(self) -> u64;
    /// Rebuilds the value from bits produced by [`Bits::to_bits`].
    fn from_bits(bits: u64) -> Self;
}

macro_rules! impl_bits_for_integers {
    ($($t:ty),* $(,)?) => {$(
        impl Bits for $t {
            fn to_bits(self) -> u64 {
                self as u64
            }
            fn from_bits(bits: u64) -> Self {
                bits as $t
            }
        }
    )*};
}

impl_bits_for_integers!(u8, u16, u32, u64, i8, i16, i32, i64);

impl Bits for bool {
    fn to_bits(self) -> u64 {
        u64::from(self)
    }
    fn from_bits(bits: u64) -> Self {
        bits != 0
    }
}

/// A typed, lock-free shared value with fractional permissions.
#[derive(Debug)]
pub struct Shared<T: Bits> {
    cell: SharedU64,
    _ty: core::marker::PhantomData<fn() -> T>,
}

impl<T: Bits> Shared<T> {
    /// Creates a shared value holding `value`.
    pub fn new(value: T) -> Self {
        Shared {
            cell: SharedU64::new(value.to_bits()),
            _ty: core::marker::PhantomData,
        }
    }

    /// Takes one unit of read permission. See [`SharedU64::try_read`].
    pub fn try_read(&self) -> Option<Read<'_, T>> {
        self.cell.try_read().map(|inner| Read {
            inner,
            _ty: core::marker::PhantomData,
        })
    }

    /// Takes the whole permission. See [`SharedU64::try_write`].
    pub fn try_write(&self) -> Option<Write<'_, T>> {
        self.cell.try_write().map(|inner| Write {
            inner,
            _ty: core::marker::PhantomData,
        })
    }
}

/// Read permission on a [`Shared`] value. Returns its unit on drop.
#[derive(Debug)]
pub struct Read<'a, T: Bits> {
    inner: ReadHandle<'a>,
    _ty: core::marker::PhantomData<fn() -> T>,
}

impl<T: Bits> Read<'_, T> {
    /// The current value.
    pub fn get(&self) -> T {
        T::from_bits(self.inner.get())
    }
}

/// The whole permission on a [`Shared`] value. Returns it on drop.
#[derive(Debug)]
pub struct Write<'a, T: Bits> {
    inner: WriteHandle<'a>,
    _ty: core::marker::PhantomData<fn() -> T>,
}

impl<T: Bits> Write<'_, T> {
    /// The current value.
    pub fn get(&self) -> T {
        T::from_bits(self.inner.get())
    }

    /// Replaces the value.
    pub fn set(&self, value: T) {
        self.inner.set(value.to_bits());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn readers_share_and_writer_excludes() {
        let cell = SharedU64::new(7);
        let r1 = cell.try_read().expect("first reader");
        let r2 = cell.try_read().expect("second reader");
        assert_eq!(r1.get(), 7);
        assert!(cell.try_write().is_none(), "writer must wait for readers");
        drop(r1);
        drop(r2);
        assert_eq!(cell.free_units(), WHOLE);
    }

    #[test]
    fn writer_excludes_readers_and_is_restored_on_drop() {
        let cell = SharedU64::new(0);
        {
            let w = cell.try_write().expect("writer");
            w.set(42);
            assert!(cell.try_read().is_none(), "reader must wait for writer");
            assert!(cell.try_write().is_none(), "writers are exclusive");
        }
        let r = cell.try_read().expect("reader after writer");
        assert_eq!(r.get(), 42);
    }

    #[test]
    fn concurrent_writers_never_both_hold_the_cell() {
        // Each writer increments under the permission. If two writers ever held
        // it at once, increments would be lost and the total would fall short.
        let cell = Arc::new(SharedU64::new(0));
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let cell = Arc::clone(&cell);
                thread::spawn(move || {
                    let mut done = 0;
                    while done < 1_000 {
                        if let Some(w) = cell.try_write() {
                            let v = w.get();
                            w.set(v + 1);
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
        let r = cell.try_read().expect("no writer is live");
        assert_eq!(r.get(), 4_000);
    }

    #[test]
    fn readers_never_observe_a_writer() {
        // A writer keeps the two halves of the value equal. A reader that
        // overlapped the writer would see them differ.
        let cell = Arc::new(SharedU64::new(0));
        let writer = {
            let cell = Arc::clone(&cell);
            thread::spawn(move || {
                let mut n = 0u64;
                while n < 2_000 {
                    if let Some(w) = cell.try_write() {
                        n += 1;
                        w.set(n);
                    } else {
                        thread::yield_now();
                    }
                }
            })
        };
        let readers: Vec<_> = (0..3)
            .map(|_| {
                let cell = Arc::clone(&cell);
                thread::spawn(move || {
                    for _ in 0..2_000 {
                        if let Some(r) = cell.try_read() {
                            let a = r.get();
                            let b = r.get();
                            assert_eq!(a, b, "value changed under a read permission");
                        }
                    }
                })
            })
            .collect();
        writer.join().unwrap();
        for r in readers {
            r.join().unwrap();
        }
    }

    #[test]
    fn typed_values_round_trip_including_signed_and_bool() {
        let i = Shared::new(-5i16);
        assert_eq!(i.try_read().unwrap().get(), -5);
        let b = Shared::new(true);
        assert!(b.try_read().unwrap().get());
        let w = Shared::new(3u8);
        w.try_write().unwrap().set(200);
        assert_eq!(w.try_read().unwrap().get(), 200);
    }
}
