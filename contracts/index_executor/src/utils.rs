use alloc::vec::Vec;
use pink_subrpc::hasher::{Blake2_256, Hasher};
use sp_io::hashing::keccak_256;
use xcm::v3::prelude::*;

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

pub fn slice_to_generalkey(key: &[u8]) -> Junction {
    let len = key.len();
    assert!(len <= 32);
    GeneralKey {
        length: len as u8,
        data: {
            let mut data = [0u8; 32];
            data[..len].copy_from_slice(key);
            data
        },
    }
}

pub fn h160_to_sr25519_pub(addr: &[u8]) -> [u8; 32] {
    Blake2_256::hash(&[b"evm:", addr].concat())
}

pub fn ss58_to_h160(addr: &[u8]) -> [u8; 20] {
    let hash = keccak_256(
        sp_runtime::AccountId32::try_from(addr)
            .expect("Invalid AccountId")
            .as_ref(),
    );
    let mut h160_address = [0u8; 20];
    h160_address.copy_from_slice(&hash[12..]);
    h160_address
}
