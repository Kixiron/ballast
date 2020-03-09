use core::{
    any::Any,
    marker::{PhantomData, PhantomPinned},
    mem, ops,
};

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

    pub(crate) unsafe fn inner(&self) -> &RootedInner {
        &*self.static_inner
    }

    pub(crate) unsafe fn inner_mut(&mut self) -> &mut RootedInner {
        &mut *self.static_inner
    }
}

impl<T: Sized + Any> ops::Deref for Rooted<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        debug_assert!(!self.is_null());
        debug_assert!(unsafe { !self.inner().is_null() });

        info!("Accessing rooted value at {:p}", self.inner().value_ptr());

        unsafe { self.inner().value().value.downcast_ref().unwrap() }
    }
}

impl<T: ?Sized + Any> Drop for Rooted<T> {
    fn drop(&mut self) {
        debug_assert!(!self.is_null());
        debug_assert!(unsafe { !self.inner().is_null() });

        trace!("Dropping value at {:p}", self.inner().value_ptr());

        unsafe {
            self.inner_mut().rooted = false;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RootedInner {
    pub(crate) value: *mut HeapValue<dyn Any>,
    pub(crate) rooted: bool,
    pub(crate) color: Color,
    pub(crate) heap: ContainingHeap,
    pub(crate) size: usize,
    pub(crate) __pinned: PhantomPinned,
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

    pub(crate) unsafe fn value(&self) -> &HeapValue<dyn Any> {
        &*self.value
    }

    #[inline]
    pub(crate) unsafe fn value_mut(&mut self) -> &mut HeapValue<dyn Any> {
        &mut *self.value
    }

    pub(crate) fn is_null(&self) -> bool {
        self.value.is_null()
    }

    pub(crate) fn value_ptr(&self) -> *mut HeapValue<dyn Any> {
        self.value
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Color {
    Black,
    Grey,
    White,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ContainingHeap {
    Eden,
    Intermediate(usize),
}

pub(crate) struct HeapValue<T: Any + ?Sized + 'static> {
    value: T,
}

impl<T> HeapValue<T> {
    pub(crate) const fn new(value: T) -> Self {
        Self { value }
    }
}
