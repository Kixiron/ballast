use super::{
    memory::{self, AllocId, HeapPointer},
    weight::Weight,
};
use crate::log;
use fxhash::FxBuildHasher;
use std::{
    alloc::{self, Layout},
    collections::HashMap,
    marker::{PhantomData, PhantomPinned},
    mem,
    pin::Pin,
    sync::atomic::AtomicBool,
};

trait RootHandler {
    fn is_rooted(&self) -> bool;
    fn size(&self) -> usize;
}

pub trait Heap {}

impl<T> Heap for T {}

struct HeapValue<T: Heap + ?Sized> {
    rooted: bool,
    color: Color,
    value: T,
}

impl<T: Heap + Sized> HeapValue<T> {
    pub fn new(value: T) -> Self {
        Self {
            rooted: true,
            color: Color::White,
            value,
        }
    }
}

impl<T> RootHandler for HeapValue<T> {
    fn is_rooted(&self) -> bool {
        self.rooted
    }

    fn size(&self) -> usize {
        mem::size_of::<HeapValue<T>>()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Color {
    Black,
    Grey,
    White,
}

#[derive(Debug, PartialEq)]
pub struct Rooted<T: ?Sized + Heap> {
    inner: *mut RootedInner<T>,
}

impl<T: Sized + Heap> std::ops::Deref for Rooted<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        log::trace!("Dropping rooted value at {:p}", self.inner);
        debug_assert!(!self.inner.is_null());

        // Safety: The validity of the contained pointers is controlled by the GC
        unsafe { &(&*(&*self.inner).inner).value }
    }
}

impl<T: ?Sized + Heap> Drop for Rooted<T> {
    fn drop(&mut self) {
        debug_assert!(!self.inner.is_null());
        debug_assert!(!(unsafe { &*self.inner }).inner.is_null());

        unsafe {
            (&mut *(&mut *self.inner).inner).rooted = false;
        }
    }
}

struct RootedInner<T: ?Sized + Heap> {
    __unpin: PhantomPinned,
    inner: *mut HeapValue<T>,
}

impl<T> RootHandler for RootedInner<T> {
    fn is_rooted(&self) -> bool {
        unsafe { (&*self.inner).is_rooted() }
    }

    fn size(&self) -> usize {
        unsafe { (&*self.inner).size() }
    }
}

struct RootHandle {
    root_inner: Pin<Box<dyn RootHandler>>,
}

impl RootHandler for RootHandle {
    fn is_rooted(&self) -> bool {
        self.root_inner.is_rooted()
    }

    fn size(&self) -> usize {
        self.root_inner.size()
    }
}

pub struct BumpHeap {
    young_start: HeapPointer,
    young_end: HeapPointer,
    young_current: HeapPointer,

    old_start: HeapPointer,
    old_end: HeapPointer,
    old_current: HeapPointer,

    roots: Vec<RootHandle>,
    next_id: AllocId,
    allocations: Vec<Pin<Box<RootedInner<dyn Heap>>>>,
}

impl BumpHeap {
    pub fn new(options: BumpOptions) -> Self {
        let layout = Layout::from_size_align(
            options.young_heap_size + options.old_heap_size,
            memory::page_size(),
        )
        .unwrap();

        let allocation = HeapPointer::new(unsafe { alloc::alloc_zeroed(layout) as usize });
        assert!(!allocation.is_null());

        let (young_start, young_current) = (allocation, allocation);
        let young_end = allocation + options.young_heap_size;

        let old_start = allocation + options.young_heap_size;
        let old_current = old_start;
        let old_end = old_start + options.old_heap_size;

        log::info!(
            "Constructed bump allocator with {}kb young heap and {}kb old heap",
            options.young_heap_size / 1024,
            options.old_heap_size / 1024,
        );

        Self {
            young_start,
            young_current,
            young_end,

            old_start,
            old_end,
            old_current,

            next_id: AllocId::new(0),
            roots: Vec::with_capacity(50),
            allocations: Vec::with_capacity(50),
        }
    }

    pub unsafe fn alloc<T: Sized + Heap + 'static>(&mut self, value: T) -> Rooted<T> {
        let allocation_size = mem::size_of::<HeapValue<T>>();
        log::trace!("Allocating object of size {}", allocation_size);

        // TODO: https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
        if self.young_current + allocation_size >= self.young_end {
            log::trace!("Young heap OOM, starting scavenge");
            self.scavenge();
        }

        let ptr = self.young_current;
        self.young_current += allocation_size;

        debug_assert!(!ptr.is_null());
        let mut inner = Box::pin(RootedInner {
            inner: ptr.as_mut_ptr::<HeapValue<T>>(),
            __unpin: PhantomPinned,
        });
        let inner_ptr = inner.as_mut().get_unchecked_mut() as *mut RootedInner<T>;

        self.roots.push(RootHandle { root_inner: inner });

        ptr.as_mut_ptr::<HeapValue<T>>()
            .write(HeapValue::new(value));

        log::trace!(
            "Allocated object successfully at {:p}",
            ptr.as_ptr::<HeapValue<T>>()
        );
        Rooted { inner: inner_ptr }
    }

    pub fn scavenge(&mut self) {}
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
        simple_logger::init();

        let mut bump = BumpHeap::new(BumpOptions::default());

        let one_hundred: Rooted<usize> = unsafe { bump.alloc(100) };
        assert_eq!(*one_hundred, 100usize);
    }
}
