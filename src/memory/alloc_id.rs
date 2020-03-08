use std::ops;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct AllocId(usize);

impl AllocId {
    #[inline]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    #[inline(always)]
    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

impl From<usize> for AllocId {
    #[inline]
    fn from(id: usize) -> Self {
        Self(id)
    }
}

impl std::fmt::Pointer for AllocId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{:p}", self.0 as *const u8)
    }
}

impl<T: Into<usize>> ops::Add<T> for AllocId {
    type Output = Self;

    fn add(self, other: T) -> Self::Output {
        Self(self.0 + other.into())
    }
}

impl<T: Into<usize>> ops::AddAssign<T> for AllocId {
    fn add_assign(&mut self, other: T) {
        self.0 += other.into()
    }
}

impl<T: Into<usize>> ops::Sub<T> for AllocId {
    type Output = Self;

    fn sub(self, other: T) -> Self::Output {
        Self(self.0 - other.into())
    }
}

impl<T: Into<usize>> ops::SubAssign<T> for AllocId {
    fn sub_assign(&mut self, other: T) {
        self.0 -= other.into()
    }
}

impl<T: Into<usize>> ops::Mul<T> for AllocId {
    type Output = Self;

    fn mul(self, other: T) -> Self::Output {
        Self(self.0 * other.into())
    }
}

impl<T: Into<usize>> ops::MulAssign<T> for AllocId {
    fn mul_assign(&mut self, other: T) {
        self.0 *= other.into()
    }
}

impl<T: Into<usize>> ops::Div<T> for AllocId {
    type Output = Self;

    fn div(self, other: T) -> Self::Output {
        Self(self.0 + other.into())
    }
}

impl<T: Into<usize>> ops::DivAssign<T> for AllocId {
    fn div_assign(&mut self, other: T) {
        self.0 /= other.into()
    }
}

impl<T: Into<usize>> ops::BitAnd<T> for AllocId {
    type Output = Self;

    fn bitand(self, other: T) -> Self::Output {
        Self(self.0 & other.into())
    }
}

impl<T: Into<usize>> ops::BitAndAssign<T> for AllocId {
    fn bitand_assign(&mut self, other: T) {
        self.0 &= other.into()
    }
}

impl<T: Into<usize>> ops::BitOr<T> for AllocId {
    type Output = Self;

    fn bitor(self, other: T) -> Self::Output {
        Self(self.0 | other.into())
    }
}

impl<T: Into<usize>> ops::BitOrAssign<T> for AllocId {
    fn bitor_assign(&mut self, other: T) {
        self.0 |= other.into()
    }
}

impl<T: Into<usize>> ops::BitXor<T> for AllocId {
    type Output = Self;

    fn bitxor(self, other: T) -> Self::Output {
        Self(self.0 ^ other.into())
    }
}

impl<T: Into<usize>> ops::BitXorAssign<T> for AllocId {
    fn bitxor_assign(&mut self, other: T) {
        self.0 ^= other.into()
    }
}

impl<T: Into<usize>> ops::Shl<T> for AllocId {
    type Output = Self;

    fn shl(self, other: T) -> Self::Output {
        Self(self.0 << other.into())
    }
}

impl<T: Into<usize>> ops::ShlAssign<T> for AllocId {
    fn shl_assign(&mut self, other: T) {
        self.0 <<= other.into()
    }
}

impl<T: Into<usize>> ops::Shr<T> for AllocId {
    type Output = Self;

    fn shr(self, other: T) -> Self::Output {
        Self(self.0 >> other.into())
    }
}

impl<T: Into<usize>> ops::ShrAssign<T> for AllocId {
    fn shr_assign(&mut self, other: T) {
        self.0 >>= other.into()
    }
}

impl ops::Not for AllocId {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl ops::Deref for AllocId {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
