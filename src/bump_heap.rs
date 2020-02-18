use super::{
    memory::{self, AllocId, HeapPointer},
    weight::Weight,
};
use crate::log;
use fxhash::FxBuildHasher;
use std::{
    alloc::{self, Layout},
    any::Any,
    collections::HashMap,
    marker::{PhantomData, PhantomPinned},
    mem,
    pin::Pin,
    ptr, raw,
    sync::atomic::AtomicBool,
};

struct HeapValue<T: Any + ?Sized + 'static> {
    rooted: bool,
    color: Color,
    size: usize,
    value: T,
}

impl<T> HeapValue<T> {
    pub fn new(value: T) -> Self {
        Self {
            rooted: true,
            color: Color::White,
            value,
            size: mem::size_of::<HeapValue<T>>(),
        }
    }

    pub fn size(&self) -> usize {
        mem::size_of::<Self>()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Color {
    Black,
    Grey,
    White,
}

#[derive(Debug, PartialEq)]
pub struct Rooted<T: ?Sized + Any> {
    inner: *mut RootedInner<T>,
}

impl<T: Sized + Any> std::ops::Deref for Rooted<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        debug_assert!(!self.inner.is_null());
        debug_assert!(!unsafe { &*self.inner }.inner.is_null());

        log::info!(
            "Accessing rooted value at {:p}",
            unsafe { &*self.inner }.inner
        );

        // Safety: The validity of the contained pointers is controlled by the GC
        unsafe { &(&*(&*self.inner).inner).value }
    }
}

impl<T: ?Sized + Any> Drop for Rooted<T> {
    fn drop(&mut self) {
        debug_assert!(!self.inner.is_null());
        debug_assert!(!unsafe { &*self.inner }.inner.is_null());

        log::trace!("Dropping value at {:p}", unsafe { &*self.inner }.inner);

        unsafe { (&mut *(&mut *self.inner).inner).rooted = false };
    }
}

struct RootedInner<T: ?Sized + Any> {
    inner: *mut HeapValue<T>,
    __unpin: PhantomPinned,
}

pub struct BumpHeap {
    young_start: HeapPointer,
    young_end: HeapPointer,
    young_current: HeapPointer,

    old_start: HeapPointer,
    old_end: HeapPointer,
    old_current: HeapPointer,

    roots: Vec<Pin<Box<RootedInner<dyn Any + 'static>>>>,
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
            "Constructed bump allocator with {}kb young generation and {}kb old generation",
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

            roots: Vec::with_capacity(50),
        }
    }

    pub unsafe fn alloc<T: Sized + Any + 'static>(&mut self, value: T) -> Rooted<T> {
        let allocation_size = mem::size_of::<HeapValue<T>>();
        log::trace!("Allocating object of size {}", allocation_size);

        // TODO: https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
        if self.young_current + allocation_size >= self.young_end {
            log::trace!("Young generation OOM, starting scavenge");
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

        self.roots.push(std::mem::transmute(inner));

        ptr.as_mut_ptr::<HeapValue<T>>()
            .write(HeapValue::new(value));

        log::trace!("Allocated object successfully at {:p}", inner_ptr);

        Rooted { inner: inner_ptr }
    }

    pub fn scavenge(&mut self) {
        log::info!("Starting a Scavenge cycle");

        let mut removal = Vec::with_capacity(self.roots.len() / 2);
        for (idx, root) in self.roots.iter_mut().enumerate() {
            log::trace!("Moving rooted object at {:p} to old generation", root.inner);

            debug_assert!(!root.inner.is_null());
            if unsafe { &*root.inner }.rooted {
                unsafe {
                    // This is cursed
                    ptr::copy(
                        root.inner as *const u8,
                        self.old_current.as_mut_ptr::<u8>(),
                        (&*root.inner).size,
                    );
                };

                let raw_root: raw::TraitObject = unsafe { mem::transmute(root.inner) };
                unsafe {
                    *root.as_mut().get_unchecked_mut() = mem::transmute(raw::TraitObject {
                        data: self.old_current.as_mut_ptr(),
                        vtable: raw_root.vtable,
                    });
                }

                self.old_current += unsafe { (&*root.inner) }.size;
            } else {
                removal.push(idx);
            }
        }

        for (offset, idx) in removal.into_iter().enumerate() {
            self.roots.remove(idx - offset);
        }

        // Zero out the young heap
        unsafe {
            self.young_start
                .as_mut_ptr::<u8>()
                .write_bytes(0x00, *self.young_end - *self.young_start);
        }
        self.young_current = self.young_start;

        log::info!("Finished Scavenge cycle");
    }
}

impl Drop for BumpHeap {
    fn drop(&mut self) {
        log::info!("Dropping Bump Heap");

        let layout =
            Layout::from_size_align(*self.old_end - *self.young_start, memory::page_size())
                .unwrap();

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
        simple_logger::init();

        let mut bump = BumpHeap::new(BumpOptions::default());

        let one_hundred: Rooted<usize> = unsafe { bump.alloc(100) };
        assert_eq!(*one_hundred, 100usize);
    }

    #[test]
    fn trigger_scavenge() {
        simple_logger::init();

        let mut bump = BumpHeap::new(BumpOptions::default());

        let i: usize = 1000;
        let rooted: Rooted<usize> = unsafe { bump.alloc(i) };
        assert_eq!(*rooted, i);

        bump.scavenge();
        assert_eq!(*rooted, i);

        drop(rooted);
        drop(bump);
    }
}
