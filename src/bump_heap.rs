use crate::{
    memory::{self, HeapPointer},
    rooted::{ContainingHeap, HeapValue, Rooted, RootedInner},
    sweep_heap::SweepHeap,
};

use alloc::{alloc::Layout, boxed::Box, vec::Vec};
use core::{
    any::Any,
    mem::{self, ManuallyDrop},
    pin::Pin,
    ptr, raw,
};

pub struct BumpHeap {
    young_start: HeapPointer,
    young_end: HeapPointer,
    young_current: HeapPointer,
    heap_size: usize,
    intermediate: ManuallyDrop<SweepHeap>,
    roots: Vec<Pin<Box<RootedInner>>>,
}

impl BumpHeap {
    pub fn new(options: BumpOptions) -> Self {
        let layout = Layout::from_size_align(
            options.young_heap_size + options.old_heap_size,
            memory::page_size(),
        )
        .unwrap();

        let allocation = unsafe {
            let allocation = HeapPointer::new(alloc::alloc::alloc_zeroed(layout) as usize);
            assert!(!allocation.is_null());
            allocation
        };

        let (young_start, young_current) = (allocation, allocation);
        let young_end = allocation + options.young_heap_size;

        info!(
            "Constructed bump allocator with {}kb young generation and {}kb old generation for a total of {}kb allocated",
            options.young_heap_size / 1024,
            options.old_heap_size / 1024,
            layout.size() / 1024,
        );

        Self {
            young_start,
            young_current,
            young_end,
            heap_size: layout.size(),
            intermediate: ManuallyDrop::new(SweepHeap::from_region(
                young_end.offset(1),
                options.old_heap_size,
            )),
            roots: Vec::with_capacity(50),
        }
    }

    pub unsafe fn alloc<T: Sized + Any + 'static>(&mut self, value: T) -> Rooted<T> {
        let allocation_size = mem::size_of::<HeapValue<T>>();
        trace!("Allocating object of size {}", allocation_size);

        // TODO: https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
        if self.young_current + allocation_size > self.young_end {
            trace!("Young generation OOM, starting scavenge");
            self.scavenge();

            if self.young_current + allocation_size > self.young_end {
                panic!("Allocation too large for young generation");
            }
        }

        let ptr = self.young_current;
        self.young_current += allocation_size;

        debug_assert!(!ptr.is_null());

        ptr.as_mut_ptr::<HeapValue<T>>()
            .write(HeapValue::new(value));

        let inner: Pin<Box<RootedInner>> = Box::pin(RootedInner::new::<T>(
            ptr.as_mut_ptr(),
            ContainingHeap::Eden,
        ));
        let rooted_ptr = inner.as_ref().get_ref() as *const _ as *mut RootedInner;

        self.roots.push(inner);

        trace!("Allocated object successfully at {:p}", rooted_ptr);

        Rooted::new(rooted_ptr)
    }

    pub fn scavenge(&mut self) {
        info!("Starting Scavenge cycle");

        let mut roots = Vec::with_capacity(self.roots.len());
        mem::swap(&mut self.roots, &mut roots);

        for mut root in roots {
            assert!(!root.is_null());
            if root.is_rooted() {
                let size = root.size();
                let ptr;

                if let Some((_ptr, pocket_size)) = self.intermediate.alloc(size) {
                    ptr = _ptr;

                    unsafe {
                        ptr::copy(root.value_ptr() as *const u8, ptr.as_mut_ptr::<u8>(), size);

                        root.as_mut().get_unchecked_mut().heap =
                            ContainingHeap::Intermediate(pocket_size);
                    }
                } else {
                    self.major();

                    if let Some((_ptr, pocket_size)) = self.intermediate.alloc(size) {
                        ptr = _ptr;

                        unsafe {
                            root.as_mut().get_unchecked_mut().heap =
                                ContainingHeap::Intermediate(pocket_size);

                            ptr::copy(root.value_ptr() as *const u8, ptr.as_mut_ptr::<u8>(), size);
                        }
                    } else {
                        panic!("Old Generation OOM");
                    }
                }

                let raw_root: raw::TraitObject = unsafe { mem::transmute(root.value_ptr()) };
                unsafe {
                    *root.as_mut().get_unchecked_mut() = RootedInner {
                        value: mem::transmute(raw::TraitObject {
                            data: ptr.as_mut_ptr(),
                            vtable: raw_root.vtable,
                        }),
                        ..root.as_ref().get_ref().clone()
                    };
                }

                self.roots.push(root);
            }
        }

        trace!("Finished processing roots");

        // Zero out the young heap
        unsafe {
            self.young_start
                .as_mut_ptr::<u8>()
                .write_bytes(0x00, *self.young_end - *self.young_start);
        }
        self.young_current = self.young_start;

        info!("Finished Scavenge cycle");
    }

    pub fn major(&mut self) {
        info!("Starting a Major cleanup cycle");

        self.intermediate.collect(&mut self.roots);

        info!("Finished a Major cleanup cycle");
    }
}

impl Drop for BumpHeap {
    fn drop(&mut self) {
        info!("Dropping Bump Heap");

        let layout = Layout::from_size_align(self.heap_size, memory::page_size()).unwrap();

        unsafe { alloc::alloc::dealloc(self.young_start.as_mut_ptr(), layout) };
    }
}

impl Default for BumpHeap {
    fn default() -> Self {
        Self::new(BumpOptions::default())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BumpOptions {
    young_heap_size: usize,
    old_heap_size: usize,
}

impl Default for BumpOptions {
    fn default() -> Self {
        Self {
            young_heap_size: 1024 * 4,
            old_heap_size: 1024 * 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        let _bump = BumpHeap::default();
    }

    #[test]
    fn allocate_no_drop() {
        let mut bump = BumpHeap::default();

        let _one: ManuallyDrop<Rooted<usize>> = unsafe { ManuallyDrop::new(bump.alloc(1)) };
    }

    #[test]
    fn allocate_no_drop_deref() {
        let mut bump = BumpHeap::default();

        let one: ManuallyDrop<Rooted<usize>> = unsafe { ManuallyDrop::new(bump.alloc(1)) };
        assert_eq!(**one, 1usize);
    }

    #[test]
    fn allocate() {
        let mut bump = BumpHeap::default();

        let one_hundred: Rooted<usize> = unsafe { bump.alloc(100) };
        assert_eq!(*one_hundred, 100usize);
    }

    #[test]
    fn trigger_scavenge() {
        let mut bump = BumpHeap::default();

        let i: usize = 1000;
        let rooted: Rooted<usize> = unsafe { bump.alloc(i) };
        assert_eq!(*rooted, i);

        bump.scavenge();
        assert_eq!(*rooted, i);
    }

    #[test]
    fn allocate_a_bunch() {
        let mut bump = BumpHeap::default();

        for i in 0..4000 {
            let rooted: Rooted<usize> = unsafe { bump.alloc(i) };
            assert_eq!(*rooted, i);
            drop(rooted);
        }
    }

    #[test]
    fn allocate_into_major() {
        let mut bump = BumpHeap::new(BumpOptions::default());

        let mut permanent = Vec::with_capacity(50);
        for i in 0..100 {
            let rooted: Rooted<usize> = unsafe { bump.alloc(i) };
            assert_eq!(*rooted, i);
            permanent.push((rooted, i));
        }

        bump.major();
        println!("here");
        for (perm, i) in &permanent {
            assert_eq!(**perm, *i);
        }
        println!("here");

        println!("here");
        for i in 0..1000 {
            let rooted: Rooted<usize> = unsafe { bump.alloc(i) };
            assert_eq!(*rooted, i);
            drop(rooted);
        }
        println!("here");

        bump.major();
        println!("here");
        for (perm, i) in permanent {
            assert_eq!(*perm, i);
            drop(perm);
        }
        println!("here");

        bump.major();
        println!("here");
    }
}
