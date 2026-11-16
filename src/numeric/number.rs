use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

pub type Flow = i64;

pub trait Number:
    Copy
    + Ord
    + Default
    + std::fmt::Debug
    + From<i8>
    + Neg<Output = Self>
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + AddAssign
    + SubAssign
{
    #[inline(always)]
    fn zero() -> Self {
        Self::from(0i8)
    }

    #[inline(always)]
    fn one() -> Self {
        Self::from(1i8)
    }

    fn to_i128(self) -> i128;
}

macro_rules! impl_number {
    ($($t:ty),* $(,)?) => {
        $(
            impl Number for $t {
                #[inline(always)]
                fn to_i128(self) -> i128 {
                    self as i128
                }
            }
        )*
    };
}

impl_number!(i16, i32, i64, i128, isize);
