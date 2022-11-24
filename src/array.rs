//! Fixed-size arrays.

use std::slice;

/// Trait for fixed size arrays.
pub unsafe trait Array {
    /// The array’s element type
    type Item;

    #[doc(hidden)]
    /// The smallest index type that indexes the array.
    type Index: Index;

    /// Returns a raw pointer to the slice's buffer.
    fn as_ptr(&self) -> *const Self::Item;

    /// Returns an unsafe mutable pointer to the slice's buffer.
    fn as_mut_ptr(&mut self) -> *mut Self::Item;

    /// Returns number of element the array can hold
    fn capacity() -> usize;

    /// Converts the array to immutable slice
    #[inline(always)]
    fn as_slice(&self) -> &[Self::Item] {
        let ptr = self as *const _ as *const _;
        unsafe { slice::from_raw_parts(ptr, Self::capacity()) }
    }

    /// Converts the array to mutable slice
    #[inline(always)]
    fn as_mut_slice(&mut self) -> &mut [Self::Item] {
        let ptr = self as *mut _ as *mut _;
        unsafe { slice::from_raw_parts_mut(ptr, Self::capacity()) }
    }
}

#[doc(hidden)]
pub trait Index: PartialEq + Copy {
    fn to_usize(self) -> usize;
    fn from(usize) -> Self;
}

impl Index for u8 {
    #[inline(always)]
    fn to_usize(self) -> usize {
        self as usize
    }

    #[inline(always)]
    fn from(ix: usize) -> Self {
        ix as u8
    }
}

impl Index for u16 {
    #[inline(always)]
    fn to_usize(self) -> usize {
        self as usize
    }

    #[inline(always)]
    fn from(ix: usize) -> Self {
        ix as u16
    }
}

impl Index for u32 {
    #[inline(always)]
    fn to_usize(self) -> usize {
        self as usize
    }

    #[inline(always)]
    fn from(ix: usize) -> Self {
        ix as u32
    }
}

impl Index for usize {
    #[inline(always)]
    fn to_usize(self) -> usize {
        self
    }

    #[inline(always)]
    fn from(ix: usize) -> Self {
        ix
    }
}

unsafe impl<T, const N: usize> Array for [T; N] {
    type Item = T;
    type Index = usize;

    fn as_ptr(&self) -> *const Self::Item {
        self.as_slice().as_ptr()
    }

    fn as_mut_ptr(&mut self) -> *mut Self::Item {
        self.as_mut_slice().as_mut_ptr()
    }

    fn capacity() -> usize {
        N
    }
}

#[cfg(feature = "use_generic_array")]

mod generic_impl {
    use super::Array;
    use generic_array::{ArrayLength, GenericArray};

    unsafe impl<T, N> Array for GenericArray<T, N>
    where
        N: ArrayLength<T>,
    {
        type Item = T;

        type Index = usize;

        #[inline(always)]
        fn as_ptr(&self) -> *const Self::Item {
            self.as_slice().as_ptr()
        }

        #[inline(always)]
        fn as_mut_ptr(&mut self) -> *mut Self::Item {
            self.as_mut_slice().as_mut_ptr()
        }

        #[inline(always)]
        fn capacity() -> usize {
            N::to_usize()
        }

        #[inline(always)]
        fn as_slice(&self) -> &[Self::Item] {
            GenericArray::as_slice(self)
        }

        #[inline(always)]
        fn as_mut_slice(&mut self) -> &mut [Self::Item] {
            GenericArray::as_mut_slice(self)
        }
    }
}
