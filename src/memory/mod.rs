use crate::log;

mod alloc_id;
mod heap_pointer;

pub use alloc_id::AllocId;
pub use heap_pointer::HeapPointer;

#[inline]
pub(crate) const fn padding_for(size: usize, align: usize) -> usize {
    let size_rounded_up = size.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);
    size_rounded_up.wrapping_sub(size)
}

#[inline(always)]
#[cfg(target_family = "unix")]
pub(crate) fn page_size() -> usize {
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;

    log::trace!("Memory Page Size: {}", size);
    assert!(size != 0);

    size
}

#[inline(always)]
#[cfg(target_family = "windows")]
pub(crate) fn page_size() -> usize {
    use std::mem::MaybeUninit;
    use winapi::um::sysinfoapi::{GetSystemInfo, SYSTEM_INFO};

    let size = unsafe {
        let mut system_info: MaybeUninit<SYSTEM_INFO> = MaybeUninit::zeroed();
        GetSystemInfo(system_info.as_mut_ptr());

        system_info.assume_init().dwPageSize as usize
    };

    log::trace!("Memory Page Size: {}", size);
    assert!(size != 0);

    size
}
