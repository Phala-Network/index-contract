use alloc::vec::Vec;
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
