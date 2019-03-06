use array::Array;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};

/// A combination of ManuallyDrop and “maybe uninitialized”;
/// this wraps a value that can be wholly or partially uninitialized;
/// it also has no drop regardless of the type of Array.
#[repr(C)] // for cast from self ptr to value
pub union MaybeUninit<A: Array> {
    empty: (),
    value: ManuallyDrop<A>,
}

impl<A: Array> MaybeUninit<A> {
    /// Create a new MaybeUninit with uninitialized interior
    pub unsafe fn uninitialized() -> Self {
        MaybeUninit { empty: () }
    }
}

impl<A: Array> Deref for MaybeUninit<A> {
    type Target = A;

    #[inline(always)]
    fn deref(&self) -> &A {
        unsafe { &self.value }
    }
}

impl<A: Array> DerefMut for MaybeUninit<A> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut A {
        unsafe { &mut self.value }
    }
}
