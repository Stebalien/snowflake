#![feature(core)]
#![cfg_attr(test, feature(test, std_misc))]
//! A module for quickly generating IDs guaranteed to be unique within the current process, for the
//! lifetime of the current process.
//!
//! 1. This module should be highly performant even on highly concurrent systems.
//! 2. While this module can run out of unique IDs, this is very unlikely assuming a sane threading
//!    model and will panic rather than potentially reusing unique IDs. 
//!
//! # Limits
//!
//! The unique ID's are `sizeof(usize) + 64` bits wide and are generated by combining a
//! `usize` global counter value with a 64bit thread local offset. This is important because each
//! thread that calls `new()` at least once will reserve at least 2^64 IDs. So, the only way to
//! run out of IDs in a reasonable amount of time is to run a 32bit system, spawn 2^32 threads, and
//! claim one ID on each thread. You might be able to do this on a 64bit system but it would take a
//! while... TL; DR: Don't create unique IDs from over 4 billion different threads on a 32bit system.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use std::{u64, usize};
use std::default::Default;

static GLOBAL_COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;

fn next_global() -> usize {
    let mut prev = GLOBAL_COUNTER.load(Ordering::Relaxed);
    loop {
        assert!(prev < usize::MAX, "Snow Crash: Go home and reevaluate your threading model!");
        let old_value = GLOBAL_COUNTER.compare_and_swap(prev, prev + 1, Ordering::Relaxed);
        if old_value == prev {
            return prev;
        } else {
            prev = old_value;
        }
    }
}

thread_local! {
    static NEXT_LOCAL_SNOWFLAKE: UnsafeCell<Snowflake> = UnsafeCell::new(Snowflake {
        prefix: next_global(),
        offset: 0
    })
}

/// An opaque unique id.
#[derive(Copy, PartialEq, Eq)]
pub struct Snowflake {
    prefix: usize,
    offset: u64,
}

impl Snowflake {
    /// Create a new unique ID.
    ///
    /// **panics** if there are no more unique IDs available. If this happens, go home and reevaluate
    /// your threading model!
    #[inline]
    pub fn new() -> Snowflake {
        NEXT_LOCAL_SNOWFLAKE.with(|snowflake| {
            unsafe {
                let next_snowflake = *snowflake.get();
                if next_snowflake.offset == u64::MAX {
                    *snowflake.get() = Snowflake {
                        prefix: next_global(),
                        offset: 0,
                    }
                } else {
                    (*snowflake.get()).offset += 1;
                }
                next_snowflake
            }
        })
    }
}

impl Default for Snowflake {
    fn default() -> Snowflake {
        Snowflake::new()
    }
}

#[cfg(test)]
mod test {
    extern crate test;
    use self::test::Bencher;
    use super::{next_global, Snowflake};
    use std::u64;

    // Glass box tests.

    #[test]
    fn test_snowflake_unthreaded() {
        let first_snowflake = Snowflake::new();
        // Not going to be able to count to u64::MAX
        { // Ignore....
            use super::NEXT_LOCAL_SNOWFLAKE;
            NEXT_LOCAL_SNOWFLAKE.with(|snowflake| {
                unsafe { (*snowflake.get()).offset = u64::MAX-10 }
            });
        } // Ignore...

        for i in (u64::MAX-11)..(u64::MAX) {
            assert!(Snowflake::new() == Snowflake { prefix: first_snowflake.prefix, offset: i+1});
        }
        let next = Snowflake::new();
        assert!(next.prefix != first_snowflake.prefix);
        assert!(next.offset == 0);
        assert!(Snowflake::new() == Snowflake { prefix: next.prefix, offset: 1 });
    }

    #[test]
    fn test_snowflake_threaded() {
        use std::sync::Future;
        let futures: Vec<Future<usize>> = (0..10).map(|_| {
            Future::spawn(move || {
                let snowflake = Snowflake::new();
                assert_eq!(snowflake.offset, 0);
                snowflake.prefix
            })
        }).collect();
        let mut results: Vec<usize> = futures.into_iter().map(|x| x.into_inner()).collect();
        results.sort();
        let old_len = results.len();
        results.dedup();
        assert_eq!(old_len, results.len());
    }

    #[bench]
    fn bench_next_global(b: &mut Bencher) {
        b.iter(|| {
            next_global();
        });
    }

    #[bench]
    fn bench_next_global_threaded(b: &mut Bencher) {
        use std::sync::TaskPool;
        use std::sync::mpsc::channel;
        let pool = TaskPool::new(4us);
        b.iter(|| {
            let (tx, rx) = channel();
            for _ in 0..4 {
                let tx = tx.clone();
                pool.execute(move || {
                    for _ in 0..1000 {
                        next_global();
                    }
                    tx.send(()).unwrap();
                });
            }
            rx.iter().take(4).count();
        });
    }

    #[bench]
    fn bench_snowflake(b: &mut Bencher) {
        b.iter(|| {
            Snowflake::new();
        });
    }

    #[bench]
    fn bench_snowflake_threaded(b: &mut Bencher) {
        use std::sync::TaskPool;
        use std::sync::mpsc::channel;
        let pool = TaskPool::new(4us);
        b.iter(|| {
            let (tx, rx) = channel();
            for _ in 0..4 {
                let tx = tx.clone();
                pool.execute(move || {
                    for _ in 0..1000 {
                        Snowflake::new();
                    }
                    tx.send(()).unwrap();
                });
            }
            rx.iter().take(4).count();
        });
    }
}
