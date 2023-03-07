use alloc::vec::Vec;
use pink_web3::ethabi::{Bytes, Uint};

pub trait ToArray<T, const N: usize> {
    fn to_array(&self) -> [T; N];
}

impl<T, const N: usize> ToArray<T, N> for Vec<T>
where
    T: Default + Copy,
{
    fn to_array(&self) -> [T; N] {
        let mut arr = [T::default(); N];
        for (a, v) in arr.iter_mut().zip(self.iter()) {
            *a = *v;
        }
        arr
    }
}

/// Evaluate `$x:expr` and if not true return `Err($y:expr)`.
///
/// Used as `ensure!(expression_to_ensure, expression_to_return_on_false)`.
#[macro_export]
macro_rules! ensure {
    ( $condition:expr, $error:expr $(,)? ) => {{
        if !$condition {
            return ::core::result::Result::Err(::core::convert::Into::into($error));
        }
    }};
}

pub trait ToBeBytes {
    fn to_be_fixed_bytes(&self) -> Bytes;
}

impl ToBeBytes for Uint {
    fn to_be_fixed_bytes(&self) -> Bytes {
        let mut a: [u8; 32] = [0; 32];
        self.to_big_endian(&mut a);
        a.into()
    }
}

/// FIXME: can be lossy
impl ToBeBytes for usize {
    fn to_be_fixed_bytes(&self) -> Bytes {
        let uint = Uint::from(*self as u32);
        uint.to_be_fixed_bytes()
    }
}
