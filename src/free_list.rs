use crate::memory::HeapPointer;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub(crate) struct FreeList {
    pub(crate) start: HeapPointer,
    pub(crate) size: usize,
    pub(crate) pockets: [Vec<HeapPointer>; NUMBER_MEMORY_POCKETS],
}

impl FreeList {
    pub const fn new(start: HeapPointer, size: usize) -> Self {
        Self {
            start,
            size,
            pockets: create_memory_pocket_array(),
        }
    }

    pub fn alloc(&mut self, size: usize) -> Option<(HeapPointer, usize)> {
        let pocket = PocketSize::next_up(size)?;

        if let Some(ptr) = self.pockets[pocket.index()].pop() {
            Some((ptr, pocket.size()))
        }
        // TODO: Is this `<=` or `<`?
        else if self.start.offset(pocket.size()) <= self.start.offset(self.size) {
            let ptr = self.start;

            self.start = self.start + pocket.size();

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
            pub fn from_usize(pocket: usize) -> Self {
                match pocket {
                    0 => PocketSize::$variant1,

                    $( var if var == PocketSize::$variant.index() => PocketSize::$variant, )*

                    var => panic!("Unrecognized pocket variant: {}", var),
                }
            }

            pub fn from_pocket_size(size: usize) -> Self {
                if size == $name1 {
                    PocketSize::$variant1
                } $( else if size == $name {
                    PocketSize::$variant
                } )* else {
                    panic!("Unrecognized pocket size: {}", size);
                }
            }

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
