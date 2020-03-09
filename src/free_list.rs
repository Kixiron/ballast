use crate::memory::HeapPointer;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct FreeList {
    pub(crate) start: HeapPointer,
    pub(crate) current: HeapPointer,
    pub(crate) size: usize,
    pub(crate) pockets: [Vec<HeapPointer>; NUMBER_MEMORY_POCKETS],
}

impl FreeList {
    pub const fn new(start: HeapPointer, size: usize) -> Self {
        Self {
            start,
            current: start,
            size,
            pockets: create_memory_pocket_array(),
        }
    }

    pub fn alloc(&mut self, size: usize) -> Option<(HeapPointer, usize)> {
        let pocket = PocketSize::next_up(size)?;
        if self.current.offset(pocket.size()) < self.start.offset(self.size) {
            let ptr = self.current;
            self.current += pocket.size();

            Some((ptr, pocket.size()))
        } else if let Some(ptr) = self.pockets[pocket.index()].pop() {
            Some((ptr, pocket.size()))
        } else {
            None
        }
    }
}

macro_rules! pocket {
    ($name1:tt: $variant1:tt = $bytes1:expr $( , $name:tt: $variant:tt = $bytes:expr )* ) => {
        const KILOBYTE: usize = 1024;

        const $name1: usize = $bytes1;
        $( const $name: usize = $bytes; )*

        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        #[repr(u8)]
        pub enum PocketSize {
            $variant1 = 0,
            $( $variant ),*
        }

        impl PocketSize {
            #[inline]
            pub fn from_usize(pocket: usize) -> Self {
                match pocket {
                    0 => PocketSize::$variant1,

                    $( var if var == PocketSize::$variant.index() => PocketSize::$variant, )*

                    var => panic!("Unrecognized pocket variant: {}", var),
                }
            }

            #[inline]
            pub fn from_pocket_size(size: usize) -> Self {
                if size == $name1 {
                    PocketSize::$variant1
                } $( else if size == $name {
                    PocketSize::$variant
                } )* else {
                    panic!("Unrecognized pocket size: {}", size);
                }
            }

            #[inline]
            pub fn next_up(size: usize) -> Option<PocketSize> {
                assert!(size >= $name1);

                if size <= $name1 {
                    Some(PocketSize::$variant1)
                } $( else if size <= $name {
                        Some(PocketSize::$variant)
                } )* else {
                    None
                }
            }

            #[inline]
            pub fn next_down(size: usize) -> Option<PocketSize> {
                assert!(size >= $name1);

                if size < $name1 {
                    Some(PocketSize::$variant1)
                } $( else if size < $name {
                        Some(PocketSize::$variant)
                } )* else {
                    None
                }
            }

            pub const fn index(&self) -> usize {
                *self as u8 as usize
            }

            pub const fn size(&self) -> usize {
                MEMORY_POCKETS[self.index()]
            }

            #[inline]
            pub fn reclaim(size: usize, ptr: HeapPointer, list: &mut FreeList) {
                let pocket = PocketSize::from_pocket_size(size);
                list.pockets[pocket.index()].push(ptr);
            }
        }

        const NUMBER_MEMORY_POCKETS: usize = [ (), $( pocket!(@replace_with_unit $name) ),* ].len();
        const MEMORY_POCKETS: [usize; NUMBER_MEMORY_POCKETS] = [ $name1 $( , $name )* ];

        const fn create_memory_pocket_array() -> [Vec<HeapPointer>; NUMBER_MEMORY_POCKETS] {
            [ Vec::new(), $( { pocket!(@replace_with_unit $name); Vec::new() } ),* ]
        }
    };

    (@replace_with_unit $( $t:tt ),* ) => {
        ()
    };
}

pocket! {
    MINI_POCKET:   Mini   = 1,
    TINY_POCKET:   Tiny   = 32,
    SMALL_POCKET:  Small  = 128,
    MEDIUM_POCKET: Medium = KILOBYTE * 2,
    LARGE_POCKET:  Large  = KILOBYTE * 8,
    HUGE_POCKET:   Huge   = KILOBYTE * 32
}
