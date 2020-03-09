//#![no_std]
#![feature(raw)]

extern crate alloc;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "log")]
#[macro_use]
extern crate log;

#[cfg(not(feature = "log"))]
#[macro_use]
mod log {
    #![allow(unused_macros)]

    macro_rules! dummy_log {
        ($($name:ident),*) => {
            dummy_log!(($) $($name),*);
        };

        (($dollar:tt) $($name:ident),*) => {
            $(
                macro_rules! $name {
                    (target: $dollar target:expr, $dollar ($dollar arg:tt)+) => {};
                    ($dollar ($dollar arg:tt)+) => {};
                }
            )*
        };
    }

    dummy_log!(debug, error, info, warn, trace);
}

mod bump_heap;
mod free_list;
mod memory;
mod rooted;
mod sweep_heap;

pub use bump_heap::BumpHeap;
