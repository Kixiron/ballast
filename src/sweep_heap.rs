use crate::{
    free_list::{FreeList, PocketSize},
    memory::{self, HeapPointer},
    rooted::{ContainingHeap, RootedInner},
};
use alloc::{boxed::Box, vec::Vec};
use core::{mem, pin::Pin, raw};

#[derive(Debug)]
pub(crate) struct SweepHeap {
    start: HeapPointer,
    size: usize,
    free_list: FreeList,
}

impl SweepHeap {
    pub fn new(size: usize) -> Self {
        let layout = alloc::alloc::Layout::from_size_align(size, memory::page_size())
            .expect("Failed to create heap layout");

        // Safety: With a valid Layout, the allocation should be successful.
        // Additionally, the pointer is checked for `null`, so the resulting pointer
        // should also be to valid memory.
        // TODO: Is it worth it to use `alloc::alloc` over `alloc::alloc_zeroed`?
        let start = HeapPointer::new(unsafe { alloc::alloc::alloc_zeroed(layout) } as usize);
        assert!(!start.is_null(), "The pointer to allocated memory is null");

        Self {
            start,
            size,
            free_list: FreeList::new(start, size),
        }
    }

    pub const fn from_region(start: HeapPointer, size: usize) -> Self {
        Self {
            start,
            size,
            free_list: FreeList::new(start, size),
        }
    }

    pub fn alloc(&mut self, size: usize) -> Option<(HeapPointer, usize)> {
        self.free_list.alloc(size)
    }

    pub fn collect(&mut self, roots: &mut Vec<Pin<Box<RootedInner>>>) {
        self.sweep(roots);

        if dbg!(self.fragmentation()) > 0.50 {
            self.compact(roots);
        }
    }

    pub fn sweep(&mut self, roots: &mut Vec<Pin<Box<RootedInner>>>) {
        roots.retain(|root| {
            if let ContainingHeap::Intermediate(pocket_size) = &root.heap {
                if !root.is_rooted() {
                    let raw_root: raw::TraitObject = unsafe { mem::transmute(root.value_ptr()) };

                    PocketSize::reclaim(
                        *pocket_size,
                        HeapPointer::new(raw_root.data as usize),
                        &mut self.free_list,
                    );

                    return false;
                }
            }

            true
        });
    }

    pub fn compact(&mut self, roots: &mut Vec<Pin<Box<RootedInner>>>) {
        for root in roots {
            if let ContainingHeap::Intermediate(pocket_size) = &root.heap {
                // TODO: Sort by low to high?
            }
        }

        todo!("Compact")
    }

    // TODO: Fragmentation's kinda wack
    #[inline]
    pub fn fragmentation(&self) -> f32 {
        1.0 - ((self.free_list.current.as_usize() - self.free_list.start.as_usize()) as f32
            / self.size as f32)
    }
}

impl Drop for SweepHeap {
    fn drop(&mut self) {
        let layout = alloc::alloc::Layout::from_size_align(self.size, memory::page_size())
            .expect("Failed to create heap layout");

        // Safety: With a valid layout and a valid `start` pointer, the deallocation should be successful
        unsafe { alloc::alloc::dealloc(self.start.as_mut_ptr(), layout) };
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_drop_heap() {
        color_backtrace::install();

        let heap = SweepHeap::new(1024 * 1000 * 1000); // Allocate 1GB
        drop(heap);
    }

    #[test]
    fn allocate_to_heap() {
        color_backtrace::install();

        let mut heap = SweepHeap::new(1024 * 1000 * 1000); // Allocate 1GB

        let id = heap.alloc::<usize>(1000).unwrap();
        assert!(heap.contains(id));
        assert_eq!(heap.get::<usize>(id).unwrap(), 1000);

        heap.write(id, 2000).unwrap();
        assert!(heap.contains(id));
        assert_eq!(heap.get::<usize>(id).unwrap(), 2000);

        drop(heap);
    }

    #[test]
    fn collection() {
        color_backtrace::install();

        let mut heap = SweepHeap::new(1024 * 1000 * 1000); // Allocate 1GB

        let id = heap.alloc::<usize>(1000).unwrap();
        assert!(heap.contains(id));
        assert_eq!(heap.get::<usize>(id).unwrap(), 1000);

        heap.collect();
        assert!(!heap.contains(id));

        drop(heap);
    }
}
*/
