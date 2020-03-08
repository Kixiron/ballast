use super::{
    memory::{self, AllocId, HeapPointer},
    weight::Weight,
};
use crate::{log, sweep_heap::SweepHeap};
use fxhash::FxBuildHasher;
use std::{
    alloc::{self, Layout},
    any::Any,
    collections::HashMap,
    marker::{PhantomData, PhantomPinned},
    mem::{self, ManuallyDrop},
    pin::Pin,
    ptr, raw,
    sync::atomic::AtomicBool,
};

pub(crate) struct HeapValue<T: Any + ?Sized + 'static> {
    value: T,
}

impl<T> HeapValue<T> {
    pub(crate) const fn new(value: T) -> Self {
        Self { value }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum Color {
    Black,
    Grey,
    White,
}

#[derive(Debug, PartialEq)]
pub struct Rooted<T: ?Sized + Any> {
    static_inner: *mut RootedInner,
    __type: PhantomData<T>,
}

impl<T: ?Sized + Any> Rooted<T> {
    pub(crate) fn new(ptr: *mut RootedInner) -> Self {
        Self {
            static_inner: ptr,
            __type: PhantomData,
        }
    }

    pub(crate) fn is_null(&self) -> bool {
        self.static_inner.is_null()
    }

    pub(crate) fn inner(&self) -> &RootedInner {
        unsafe { &*self.static_inner }
    }

    pub(crate) fn inner_mut(&mut self) -> &mut RootedInner {
        unsafe { &mut *self.static_inner }
    }
}

impl<T: Sized + Any> std::ops::Deref for Rooted<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        debug_assert!(!self.is_null());
        debug_assert!(!self.inner().is_null());

        log::info!("Accessing rooted value at {:p}", self.inner().value_ptr());

        self.inner().value().value.downcast_ref().unwrap()
    }
}

impl<T: ?Sized + Any> Drop for Rooted<T> {
    fn drop(&mut self) {
        debug_assert!(!self.is_null());
        debug_assert!(!self.inner().is_null());

        log::trace!("Dropping value at {:p}", self.inner().value_ptr());

        self.inner_mut().rooted = false;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ContainingHeap {
    Eden,
    Intermediate(usize),
}

#[derive(Debug, Clone)]
pub(crate) struct RootedInner {
    value: *mut HeapValue<dyn Any>,
    rooted: bool,
    color: Color,
    heap: ContainingHeap,
    size: usize,
    __pinned: PhantomPinned,
}

impl RootedInner {
    pub(crate) fn new<T: Any + 'static>(ptr: *mut HeapValue<T>, heap: ContainingHeap) -> Self {
        Self {
            value: ptr,
            rooted: true,
            color: Color::White,
            heap,
            size: mem::size_of::<HeapValue<T>>(),
            __pinned: PhantomPinned,
        }
    }

    pub(crate) const fn size(&self) -> usize {
        self.size
    }

    pub(crate) const fn is_rooted(&self) -> bool {
        self.rooted
    }

    pub(crate) const fn color(&self) -> Color {
        self.color
    }

    pub(crate) const fn containing_heap(&self) -> ContainingHeap {
        self.heap
    }

    pub(crate) fn value(&self) -> &HeapValue<dyn Any> {
        unsafe { &*self.value }
    }

    #[inline]
    pub(crate) fn value_mut(&mut self) -> &mut HeapValue<dyn Any> {
        unsafe { &mut *self.value }
    }

    pub(crate) fn is_null(&self) -> bool {
        self.value.is_null()
    }

    pub(crate) fn value_ptr(&self) -> *mut HeapValue<dyn Any> {
        self.value
    }
}

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

        let total_heap_size = layout.size() + memory::padding_for(layout.size(), layout.align());

        let allocation = HeapPointer::new(unsafe { alloc::alloc_zeroed(layout) as usize });
        assert!(!allocation.is_null());

        let (young_start, young_current) = (allocation, allocation);
        let young_end = (young_start + total_heap_size) - options.old_heap_size;

        log::info!(
            "Constructed bump allocator with {}kb young generation and {}kb old generation for a total of {}kb allocated",
            (*young_end - *young_start) / 1024,
            options.old_heap_size / 1024,
            total_heap_size / 1024,
        );

        Self {
            young_start,
            young_current,
            young_end,
            heap_size: total_heap_size,
            intermediate: ManuallyDrop::new(SweepHeap::from_region(
                young_end.offset(1),
                options.old_heap_size,
            )),
            roots: Vec::with_capacity(50),
        }
    }

    pub unsafe fn alloc<T: Sized + Any + 'static>(&mut self, value: T) -> Rooted<T> {
        let allocation_size = mem::size_of::<HeapValue<T>>();
        log::trace!("Allocating object of size {}", allocation_size);

        // TODO: https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
        if self.young_current + allocation_size > self.young_end {
            log::trace!("Young generation OOM, starting scavenge");
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

        log::trace!("Allocated object successfully at {:p}", rooted_ptr);

        Rooted::new(rooted_ptr)
    }

    pub fn scavenge(&mut self) {
        log::info!("Starting Scavenge cycle");

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
                        root.as_mut().get_unchecked_mut().heap =
                            ContainingHeap::Intermediate(pocket_size);

                        ptr::copy(root.value_ptr() as *const u8, ptr.as_mut_ptr::<u8>(), size);
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

        log::trace!("Finished processing roots");

        // Zero out the young heap
        unsafe {
            self.young_start
                .as_mut_ptr::<u8>()
                .write_bytes(0x00, *self.young_end - *self.young_start);
        }
        self.young_current = self.young_start;

        log::info!("Finished Scavenge cycle");
    }

    pub fn major(&mut self) {
        log::info!("Starting a Major cleanup cycle");

        self.intermediate.collect(&mut self.roots);

        log::info!("Finished a Major cleanup cycle");
    }
}

impl Drop for BumpHeap {
    fn drop(&mut self) {
        log::info!("Dropping Bump Heap");

        let layout = Layout::from_size_align(self.heap_size, memory::page_size()).unwrap();

        unsafe { alloc::dealloc(self.young_start.as_mut_ptr(), layout) };
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BumpAllocation {
    pub(crate) ptr: HeapPointer,
    pub(crate) size: usize,
    pub(crate) children: Vec<AllocId>,
}

impl BumpAllocation {
    pub const fn new(ptr: HeapPointer, size: usize) -> Self {
        Self {
            ptr,
            size,
            children: Vec::new(),
        }
    }

    pub const fn size(&self) -> usize {
        self.size
    }

    pub const fn ptr(&self) -> HeapPointer {
        self.ptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate() {
        let mut bump = BumpHeap::new(BumpOptions::default());

        let one_hundred: Rooted<usize> = unsafe { bump.alloc(100) };
        assert_eq!(*one_hundred, 100usize);
    }

    #[test]
    fn trigger_scavenge() {
        let mut bump = BumpHeap::new(BumpOptions::default());

        let i: usize = 1000;
        let rooted: Rooted<usize> = unsafe { bump.alloc(i) };
        assert_eq!(*rooted, i);

        bump.scavenge();
        assert_eq!(*rooted, i);
    }

    #[test]
    fn allocate_a_bunch() {
        let mut bump = BumpHeap::new(BumpOptions::default());

        for i in 0..4000 {
            let rooted: Rooted<usize> = unsafe { bump.alloc(i) };
            assert_eq!(*rooted, i);
            drop(rooted);
        }
    }

    #[test]
    fn allocate_into_major() {
        simple_logger::init();

        let mut bump = BumpHeap::new(BumpOptions::default());

        let mut roots = Vec::with_capacity(4000);
        for i in 0..4000 {
            let rooted: Rooted<usize> = unsafe { bump.alloc(i) };
            assert_eq!(*rooted, i);
            roots.push((rooted, i));
        }

        for (rooted, i) in roots {
            assert_eq!(*rooted, i);
        }
    }
}
