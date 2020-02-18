#![feature(raw)]

use std::alloc::Layout;

#[cfg(feature = "log")]
mod log {
    pub use log::{debug, error, info, trace, warn};
}

#[cfg(not(feature = "log"))]
mod log {
    macro_rules! dummy_log {
        ($($name:ident),*) => {
            $(
                macro_rules! $name {
                    (target: $target:expr, $($arg:tt)+) => {};
                    ($($arg:tt)+) => {};
                }
            )*
        };
    }

    dummy_log!(debug, error, info, trace, warn);
}

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod bump_heap;
mod free_list;
mod memory;
mod sweep_heap;
mod weight;

pub use bump_heap::BumpHeap;
use free_list::*;
use memory::*;
pub use sweep_heap::SweepHeap;
use weight::*;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Error {
    OutOfSpace = 0,
    SizeMisalign,
    NoAllocationFound,
}

// #[derive(Debug)]
// pub struct Ballast {
//     old_heap: Heap,
//     medium_heap: Heap,
//     anchors: (),
// }

#[derive(Debug, Copy, Clone)]
pub struct Options {
    pub heap_size: usize,
    pub num_threads: usize,
}

pub trait BallastGc {
    type Allocated;
    type Deallocated;

    fn new(options: Options) -> Self;
    fn alloc(&mut self, layout: Layout) -> Self::Allocated;
    fn dealloc(&mut self, dealloc: Self::Deallocated, layout: Layout);
}
