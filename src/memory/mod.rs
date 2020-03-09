mod heap_pointer;

pub use heap_pointer::HeapPointer;

#[inline]
pub(crate) const fn padding_for(size: usize, align: usize) -> usize {
    let size_rounded_up = size.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);
    size_rounded_up.wrapping_sub(size)
}

#[inline(always)]
#[cfg(all(target_family = "unix", not(miri)))]
pub(crate) fn page_size() -> usize {
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;

    trace!("Memory Page Size: {}", size);
    assert!(size != 0);

    size
}

#[cfg(miri)]
pub(crate) fn page_size() -> usize {
    4096
}

#[inline(always)]
#[cfg(all(target_family = "windows", not(miri)))]
pub(crate) fn page_size() -> usize {
    use core::mem::MaybeUninit;
    use winapi::um::sysinfoapi::{GetSystemInfo, SYSTEM_INFO};

    let size = unsafe {
        let mut system_info: MaybeUninit<SYSTEM_INFO> = MaybeUninit::zeroed();
        GetSystemInfo(system_info.as_mut_ptr());

        system_info.assume_init().dwPageSize as usize
    };

    trace!("Memory Page Size: {}", size);
    assert!(size != 0);

    size
}
