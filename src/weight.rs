use crate::{AllocId, HeapPointer, PocketSize};
use fxhash::FxBuildHasher;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Weight {
    pub(crate) ptr: HeapPointer,
    pub(crate) size: usize,
    pub(crate) children: Vec<AllocId>,
    pub(crate) shade: Shade,
    pub(crate) pocket: PocketSize,
}

impl Weight {
    pub const fn new(ptr: HeapPointer, size: usize, pocket: PocketSize) -> Self {
        Self {
            ptr,
            size,
            children: Vec::new(),
            shade: Shade::White,
            pocket,
        }
    }

    pub const fn size(&self) -> usize {
        self.size
    }

    pub const fn ptr(&self) -> HeapPointer {
        self.ptr
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Shade {
    White = 0,
    Grey,
    Black,
}

impl Shade {
    pub fn is_white(&self) -> bool {
        *self == Self::White
    }

    pub fn is_grey(&self) -> bool {
        *self == Self::Grey
    }

    pub fn is_black(&self) -> bool {
        *self == Self::Black
    }
}
