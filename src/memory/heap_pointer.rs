use std::ops;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct HeapPointer(usize);

impl HeapPointer {
    #[inline]
    pub const fn new(ptr: usize) -> Self {
        Self(ptr)
    }

    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    #[inline]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    #[inline]
    pub const fn offset(self, offset: usize) -> Self {
        Self(self.0 + offset)
    }

    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_non_null(self) -> bool {
        self.0 != 0
    }

    #[inline]
    pub fn as_ref<'a, T>(self) -> &'a T {
        debug_assert!(!self.is_null());

        unsafe { &*(self.0 as *const T) }
    }

    #[inline]
    pub fn as_mut<'a, T>(self) -> &'a mut T {
        debug_assert!(!self.is_null());

        unsafe { &mut *(self.0 as *mut T) }
    }
}

impl<T: Into<usize>> ops::Add<T> for HeapPointer {
    type Output = Self;

    fn add(self, other: T) -> Self::Output {
        Self(self.0 + other.into())
    }
}

impl<T: Into<usize>> ops::AddAssign<T> for HeapPointer {
    fn add_assign(&mut self, other: T) {
        self.0 += other.into()
    }
}

impl<T: Into<usize>> ops::Sub<T> for HeapPointer {
    type Output = Self;

    fn sub(self, other: T) -> Self::Output {
        Self(self.0 - other.into())
    }
}

impl<T: Into<usize>> ops::SubAssign<T> for HeapPointer {
    fn sub_assign(&mut self, other: T) {
        self.0 -= other.into()
    }
}

impl<T: Into<usize>> ops::Mul<T> for HeapPointer {
    type Output = Self;

    fn mul(self, other: T) -> Self::Output {
        Self(self.0 * other.into())
    }
}

impl<T: Into<usize>> ops::MulAssign<T> for HeapPointer {
    fn mul_assign(&mut self, other: T) {
        self.0 *= other.into()
    }
}

impl<T: Into<usize>> ops::Div<T> for HeapPointer {
    type Output = Self;

    fn div(self, other: T) -> Self::Output {
        Self(self.0 + other.into())
    }
}

impl<T: Into<usize>> ops::DivAssign<T> for HeapPointer {
    fn div_assign(&mut self, other: T) {
        self.0 /= other.into()
    }
}

impl<T: Into<usize>> ops::BitAnd<T> for HeapPointer {
    type Output = Self;

    fn bitand(self, other: T) -> Self::Output {
        Self(self.0 & other.into())
    }
}

impl<T: Into<usize>> ops::BitAndAssign<T> for HeapPointer {
    fn bitand_assign(&mut self, other: T) {
        self.0 &= other.into()
    }
}

impl<T: Into<usize>> ops::BitOr<T> for HeapPointer {
    type Output = Self;

    fn bitor(self, other: T) -> Self::Output {
        Self(self.0 | other.into())
    }
}

impl<T: Into<usize>> ops::BitOrAssign<T> for HeapPointer {
    fn bitor_assign(&mut self, other: T) {
        self.0 |= other.into()
    }
}

impl<T: Into<usize>> ops::BitXor<T> for HeapPointer {
    type Output = Self;

    fn bitxor(self, other: T) -> Self::Output {
        Self(self.0 ^ other.into())
    }
}

impl<T: Into<usize>> ops::BitXorAssign<T> for HeapPointer {
    fn bitxor_assign(&mut self, other: T) {
        self.0 ^= other.into()
    }
}

impl<T: Into<usize>> ops::Shl<T> for HeapPointer {
    type Output = Self;

    fn shl(self, other: T) -> Self::Output {
        Self(self.0 << other.into())
    }
}

impl<T: Into<usize>> ops::ShlAssign<T> for HeapPointer {
    fn shl_assign(&mut self, other: T) {
        self.0 <<= other.into()
    }
}

impl<T: Into<usize>> ops::Shr<T> for HeapPointer {
    type Output = Self;

    fn shr(self, other: T) -> Self::Output {
        Self(self.0 >> other.into())
    }
}

impl<T: Into<usize>> ops::ShrAssign<T> for HeapPointer {
    fn shr_assign(&mut self, other: T) {
        self.0 >>= other.into()
    }
}

impl ops::Not for HeapPointer {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl ops::Deref for HeapPointer {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<usize> for HeapPointer {
    #[inline]
    fn from(ptr: usize) -> Self {
        Self(ptr)
    }
}

impl<T> From<*mut T> for HeapPointer {
    #[inline]
    fn from(ptr: *mut T) -> Self {
        Self(ptr as usize)
    }
}

impl<T> From<*const T> for HeapPointer {
    #[inline]
    fn from(ptr: *const T) -> Self {
        Self(ptr as usize)
    }
}
