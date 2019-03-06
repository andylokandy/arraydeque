use super::nodrop::NoDrop;
use array::Array;
use std::mem;
use std::ops::{Deref, DerefMut};

/// A combination of NoDrop and “maybe uninitialized”;
/// this wraps a value that can be wholly or partially uninitialized.
///
/// NOTE: This is known to not be a good solution, but it's the one we have kept
/// working on stable Rust. Stable improvements are encouraged, in any form,
/// but of course we are waiting for a real, stable, MaybeUninit.
#[repr(C)] // for cast from self ptr to value
pub struct MaybeUninit<T>(NoDrop<T>);
// why don't we use ManuallyDrop here: It doesn't inhibit
// enum layout optimizations that depend on T, and we support older Rust.

impl<T> MaybeUninit<T> {
    /// Create a new MaybeUninit with uninitialized interior
    pub unsafe fn uninitialized() -> Self {
        Self(NoDrop::new(mem::uninitialized()))
    }
}

impl<A: Array> Deref for MaybeUninit<A> {
    type Target = A;

    #[inline(always)]
    fn deref(&self) -> &A {
        &self.0
    }
}

impl<A: Array> DerefMut for MaybeUninit<A> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut A {
        &mut self.0
    }
}
