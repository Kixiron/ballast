use super::{
    free_list::*,
    memory::{self, AllocId, HeapPointer},
    weight::*,
    Error, Result,
};
use fxhash::FxBuildHasher;
use std::{
    alloc::{self, Layout},
    collections::{HashMap, HashSet},
    mem, ptr, slice,
};

#[derive(Debug)]
pub struct SweepHeap {
    start: HeapPointer,
    size: usize,
    free_list: FreeList,
    roots: Vec<AllocId>,
    white: HashMap<AllocId, Weight, FxBuildHasher>,
    grey: HashMap<AllocId, Weight, FxBuildHasher>,
    black: HashMap<AllocId, Weight, FxBuildHasher>,
    next_id: AllocId,
}

impl SweepHeap {
    pub fn new(size: usize) -> Self {
        let layout = alloc::Layout::from_size_align(size, memory::page_size())
            .expect("Failed to create heap layout");

        // Safety: With a valid Layout, the allocation should be successful.
        // Additionally, the pointer is checked for `null`, so the resulting pointer
        // should also be to valid memory.
        // TODO: Is it worth it to use `alloc::alloc` over `alloc::alloc_zeroed`?
        let start = HeapPointer::new(unsafe { alloc::alloc_zeroed(layout) } as usize);
        assert!(!start.is_null(), "The pointer to allocated memory is null");

        Self {
            start,
            size,
            free_list: FreeList::new(start, size),
            roots: Vec::with_capacity(50),
            white: HashMap::with_hasher(FxBuildHasher::default()),
            grey: HashMap::with_hasher(FxBuildHasher::default()),
            black: HashMap::with_hasher(FxBuildHasher::default()),
            next_id: AllocId::new(0),
        }
    }

    pub fn alloc<T: std::fmt::Debug>(&mut self, data: T) -> Option<AllocId> {
        let (ptr, pocket_size) = self.free_list.alloc(mem::size_of::<T>())?;
        let id = self.next_id;
        self.next_id += 1usize;

        // Safety: The pointer provided by `FreeList::alloc` is trusted to be valid
        unsafe { ptr.as_mut_ptr::<T>().write(data) };

        self.white.insert(
            id,
            Weight::new(
                ptr,
                mem::size_of::<T>(),
                PocketSize::from_pocket_size(pocket_size),
            ),
        );

        Some(id)
    }

    pub fn write<T>(&self, id: AllocId, data: T) -> Result<()> {
        let ptr = self.get_ptr(id).ok_or(Error::NoAllocationFound)?;

        // Safety: The pointer stored in one of the various hashmaps should be up-to-date
        unsafe { ptr.as_mut_ptr::<T>().write(data) };

        Ok(())
    }

    pub fn get<T>(&self, id: AllocId) -> Result<T> {
        let weight = self.get_weight(id).ok_or(Error::NoAllocationFound)?;
        // TODO: `<=` might be alright
        if weight.size() != mem::size_of::<T>() {
            return Err(Error::SizeMisalign);
        }

        // TODO: Should `MaybeUninit::zeroed()` be used?
        let mut output = mem::MaybeUninit::uninit();
        // Safety: The pointers contained in the heap should be A: Non-null and B: Valid
        unsafe { ptr::copy_nonoverlapping(weight.ptr().as_mut_ptr(), output.as_mut_ptr(), 1) };

        // Safety: If the copy happens, then the output should be initialized
        // to the value at `weight.ptr`
        Ok(unsafe { output.assume_init() })
    }

    #[inline]
    fn get_weight(&self, id: AllocId) -> Option<&Weight> {
        if let Some(weight) = self.black.get(&id) {
            Some(weight)
        } else if let Some(weight) = self.grey.get(&id) {
            Some(weight)
        } else if let Some(weight) = self.white.get(&id) {
            Some(weight)
        } else {
            None
        }
    }

    #[inline]
    fn get_weight_mut(&mut self, id: AllocId) -> Option<&mut Weight> {
        if let Some(weight) = self.black.get_mut(&id) {
            Some(weight)
        } else if let Some(weight) = self.grey.get_mut(&id) {
            Some(weight)
        } else if let Some(weight) = self.white.get_mut(&id) {
            Some(weight)
        } else {
            None
        }
    }

    #[inline]
    pub fn contains(&self, id: AllocId) -> bool {
        self.black.contains_key(&id) || self.grey.contains_key(&id) || self.white.contains_key(&id)
    }

    #[inline]
    fn get_ptr(&self, id: AllocId) -> Option<HeapPointer> {
        Some(self.get_weight(id)?.ptr())
    }

    pub fn collect(&mut self) {
        self.sweep();

        if self.fragmentation() > 50.0 {
            self.compact();
        }
    }

    pub fn sweep(&mut self) {
        let mut to_mark = Vec::with_capacity(self.grey.len() + self.white.len() / 3);
        to_mark.extend_from_slice(&self.grey.keys().copied().collect::<Vec<_>>());

        while let Some(id) = to_mark.pop() {
            let weight = self.get_weight_mut(id).unwrap();
            weight.shade = Shade::Black;

            to_mark.extend_from_slice(&weight.children)
        }

        for (_id, weight) in self.white.drain() {
            self.free_list.reclaim(weight);
        }
    }

    pub fn compact(&mut self) {}

    pub fn root(&mut self, id: AllocId) -> Result<()> {
        self.get_weight_mut(id)
            .ok_or(Error::NoAllocationFound)?
            .shade = Shade::Grey;
        self.roots.push(id);

        Ok(())
    }

    #[inline]
    pub fn fragmentation(&self) -> f32 {
        1.0 - (self.free_list.start.as_usize() as f32 / self.size as f32)
    }
}

impl Drop for SweepHeap {
    fn drop(&mut self) {
        let layout = alloc::Layout::from_size_align(self.size, memory::page_size())
            .expect("Failed to create heap layout");

        // Safety: With a valid layout and a valid `start` pointer, the deallocation should be successful
        unsafe { alloc::dealloc(self.start.as_mut_ptr(), layout) };
    }
}

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
