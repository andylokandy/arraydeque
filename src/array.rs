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
    fn from(_: usize) -> Self;
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

macro_rules! fix_array_impl {
    ($index_type:ty, $len:expr) => {
        unsafe impl<T> Array for [T; $len] {
            type Item = T;
        
            type Index = $index_type;
        
            #[inline(always)]
            fn as_ptr(&self) -> *const T {
                self as *const _ as *const T
            }
        
            #[inline(always)]
            fn as_mut_ptr(&mut self) -> *mut T {
                self as *mut _ as *mut T
            }
        
            #[inline(always)]
            fn capacity() -> usize {
                $len
            }
        }
    };
}

macro_rules! fix_array_impl_recursive {
    ($index_type:ty, ) => ();
    ($index_type:ty, $len:expr, $($more:expr,)*) => (
        fix_array_impl!($index_type, $len);
        fix_array_impl_recursive!($index_type, $($more,)*);
    );
}

fix_array_impl_recursive!(
    u8, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
    25, 26, 27, 28, 29, 30, 31, 32, 40, 48, 50, 56, 64, 72, 96, 100, 128, 160, 192, 200, 224,
);

fix_array_impl_recursive!(u16, 256, 384, 512, 768, 1024, 2048, 4096, 8192, 16384, 32768,);

// This array size doesn't exist on 16-bit
#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
fix_array_impl_recursive!(u32, 1 << 16,);

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
